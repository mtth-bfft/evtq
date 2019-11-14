use std::collections::HashMap;

/*
 * When querying a backup file, not specifying a channel (either in <Query Path="MyChannelName">
 * or in <Query><Select Path="MyChannelName">) works with the API. That's not the case when
 * querying events from a live host: a path is required in at least one of them. Furthermore,
 * there's an implementation limitation in the EventLog API: there's no way to EvtNext() on a
 * EvtQuery() with more than 256 <Query Path="A"> or <Select Path="A"> XML filters. Consequently,
 * we have to use the EvtSubscribe() API on each channel independently, each channel with its own
 * filter.
 *
 * If live_all_channels is not set (e.g. reading a backup file), this function returns filters like:
 * "*" => <QueryList>
 *     <Query Id="0">
 *         <Select>Event[System/Channel/@Name="Security"][System/EventID=4688]</Select>
 *         <Select>Event[System/Channel/@Name="Security"][System/EventID=4624]</Select>
 *         <Select>Event[System/Channel/@Name="Application"][System/EventID=1001]</Select>
 *         <Select>Event[System/Channel/@Name="System"]</Select>
 *     </Query>
 * </QueryList>
 *
 * With live_all_channels set to the list of all channels (e.g. when live querying a remote host),
 * the same filters looks like:
 * "Security" => Some(<QueryList>
 *    <Query Id="0" Path="Security">
 *         <Select>Event[System/EventID=4688]</Select>
 *         <Select>Event[System/EventID=4624]</Select>
 *     </Query>
 * </QueryList>)
 * "Application" => Some(<QueryList>
 *    <Query Id="0" Path="Security">
 *         <Select>Event[System/EventID=1001]</Select>
 *     </Query>
 * </QueryList>)
 * "System" => None
 *
 * Filters are heavily restricted in the XPath functions they can use.
 * See https://docs.microsoft.com/en-us/windows/win32/wes/consuming-events#xpath-10-limitations
 */
pub fn xml_query_from_filters(includes: &[&str], excludes: &[&str], live_all_channels: Option<&Vec<String>>) -> Result<HashMap<String,Option<String>>, String> {
    let mut per_channel_filters : HashMap<String,Vec<String>> = HashMap::new();

    for (option_array, xml_type) in vec![(includes, "Select"), (excludes, "Suppress")] {
        for argv in option_array {
            let mut s: Vec<&str> = argv.split('/').collect();
            while s.len() < 4 {
                s.push("*");
            }
            let (channel, provider, eventid, version) = match s[..] {
                [c, p, e, v] => (c, p, e, v),
                _ => return Err(format!("Too many / separators in filter '{}'", argv)),
            };
            if (!channel.eq("*") && channel.contains("*")) ||
                (!provider.eq("*") && provider.contains("*")) ||
                (!eventid.eq("*") && eventid.contains("*")) ||
                (!version.eq("*") && version.contains("*")) {
                return Err(format!("The eventlog query API does not support * wildcards inside values"));
            }

            let tmp_vec: Vec<String>;
            let channels = if channel.eq("*") {
                match live_all_channels {
                    Some(all_names) => all_names,
                    None => {
                        tmp_vec = vec!["*".to_owned()];
                        &tmp_vec
                    },
                }
            } else {
                tmp_vec = vec![channel.to_owned()];
                &tmp_vec
            };
            for channel in channels {
                let mut xpath_query = String::new();
                if live_all_channels.is_none() && !channel.eq("*") {
                    // We're not live (e.g. reading a backup) and a specific channel is queried
                    // We can't use Path="TheChannelName" because the backup API would then return
                    // eventlogs from the local host's channel with that name
                    xpath_query.push_str(&format!(r#"[System/Channel/@Name="{}"]"#, channel));
                }
                if !provider.eq("*") {
                    xpath_query.push_str(&format!(r#"[System/Provider/@Name="{}"]"#, provider.replace("'", "\\'")));
                }
                if !eventid.eq("*") {
                    let eventid = match eventid.parse::<u16>() {
                        Ok(u) => u,
                        Err(e) => return Err(format!("Invalid EventID in filter '{}': {}", argv, e)),
                    };
                    xpath_query.push_str(&format!(r#"[System[EventID={}]]"#, eventid));
                }
                if !version.eq("*") {
                    let version = match version.parse::<u8>() {
                        Ok(u) => u,
                        Err(e) => return Err(format!("Invalid Version in filter '{}': {}", argv, e)),
                    };
                    xpath_query.push_str(&format!(r#"[System/Version={}]"#, version));
                }

                if xpath_query.len() == 0 {
                    if xml_type.eq("Select") {
                        // Ensure the channel is at least in the hashmap, to register the channel name
                        per_channel_filters.entry(channel.to_owned()).or_insert(vec![]);
                    }
                    else {
                        // Remove the channel from the include list altogether
                        per_channel_filters.remove(channel);
                    }
                    continue;
                }
                let filters = per_channel_filters.entry(channel.to_owned()).or_insert(vec![]);
                if filters.len() == 0 && xml_type == "Suppress" {
                    // When adding a <Select> to an entire channel without XPath query, we remove
                    // the XML node and just subscribe to the channel without filter. However, if
                    // we try to add a <Suppress> without an associated <Select>, it fails.
                    filters.push("<Select>Event</Select>".to_owned());
                }
                let xml_node = if live_all_channels.is_some() {
                    format!("<{} Path=\"{}\">Event{}</{}>", xml_type, channel, xpath_query, xml_type)
                } else {
                    format!("<{}>Event{}</{}>", xml_type, xpath_query, xml_type)
                };
                filters.push(xml_node);
            }
        }
    }

    let mut per_channel_xml : HashMap<String,Option<String>> = HashMap::new();
    for (channel, xml_nodes) in per_channel_filters {
        if xml_nodes.len() == 0 {
            per_channel_xml.insert(channel, None);
            continue;
        }
        let filter = if live_all_channels.is_some() {
            format!("<QueryList><Query Id=\"0\" Path=\"{}\">\n  {}\n</Query></QueryList>",
                    channel, xml_nodes.join("\n  "))
        } else {
            format!("<QueryList><Query Id=\"0\">\n  {}\n</Query></QueryList>",
                    xml_nodes.join("\n  "))
        };
        per_channel_xml.insert(channel, Some(filter));
    }
    Ok(per_channel_xml)
}