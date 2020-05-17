// Bindings auto-generated and imported from WinAPI are not expected
// to follow Rust's naming convention
#![allow(non_upper_case_globals)]

extern crate clap;
extern crate winapi;
extern crate serde;
use clap::{Arg, App, ArgGroup};
use std::result::Result;
use std::io;
use std::time::Instant;
use std::collections::BTreeMap;
use std::vec::Vec;
use std::fs::OpenOptions;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::log::*;
use crate::windows::{EvtHandle, RpcCredentials};
use crate::xml::render_event_xml;
use crate::json::render_event_json;
use crate::formatting::CommonEventProperties;
use crate::metadata::*;
use crate::csv::render_event_csv;
use crate::output_cols::{OutputColumn, parse_column_names};
use crate::filtering::xml_query_from_filters;

#[macro_use]
mod log;
mod windows;
mod xml;
mod json;
mod csv;
mod metadata;
mod output_cols;
mod formatting;
mod filtering;

pub struct RenderingConfig {
    render_callback: fn(&EvtHandle, &CommonEventProperties, &RenderingConfig) -> Result<(), String>,
    output_file: Box<Mutex<dyn std::io::Write>>,
    datefmt: String,
    metadata: Metadata,
    field_separator: char,
    json_pretty: bool,
    columns: Vec<OutputColumn>,
    rendering_start: Instant,
    event_counter: AtomicU64,
}

fn main() {
    std::process::exit(match run() {
        Ok(_) => 0,
        Err(e) => { eprintln!(" [!] Error: {}", e); 1 },
    });
}

pub fn run() -> Result<(), String> {
    let args = App::new("evtq")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Windows EventLog fetcher, parser, filter and formatter")
        .help(r#"
evtq.exe [input] [filtering] [output] [common]
Windows EventLog querying/parsing/formatting - https://github.com/mtth-bfft/evtq

INPUT:
    --from-host [URI, default is localhost]  Read events as they happen on a live host via RPC
                                             URI format: domain/username:password@hostname
    --from-backup <filename.evt(x)> Read events from a backup .evtx or .evt
    --dump-existing                 Also process existing (past) events from the queried host
    --no-wait                       Don't wait for future events to arrive from the queried host
    --list-channels                 Don't dump events, just list available channels from the host

FILTERING:
 -i --include <filter>              Only render events matching this filter (default: */*/*/*)
 -e --exclude <filter>              Don't render events matching this filter
      Filter format: ChannelName/ProviderName/EventID/Version
      Each of the four parts can be replaced with * as a wildcard

OUTPUT:
    --to-json [output.json]         Render events to a JSON file with field names (default: stdout)
    --to-xml  [output.xml]          Render events as lines of raw XML (default: stdout)
    --to-csv  [output.csv]          Render events as lines of comma-separated columns (default: stdout)
    --to-tsv  [output.tsv]          Render events as lines of tab-separated columns (default: stdout)
    --json-pretty                   Add spaces and line feeds to JSON outputs
 -a --append                        Don't overwrite output files if they exist

COMMON:
 -h --help                          Display this help text
 -V --version                       Display the current version
 -v --verbose                       Increase verbosity (can be repeated for extra information)
 -O --columns                       Comma-separated list of columns to output in JSON, CSV, or TSV
      (default: hostname,recordid,timestamp,provider,eventid,version,level_name,task_name,
                keyword_names,formatted_message,variant1,...,variant15)
      (possible fields: hostname           provider              level         level_name
                        recordid           eventid               task          task_name
                        timestamp          version               opcode        opcode_name
                        formatted_message  unformatted_message   keywords      keyword_names
                        variant1           variant2              variant3  ..  variant15
    --no-system-metadata            Don't load field names, types, and message strings from the live OS
    --export-metadata <meta.json>   Export metadata to file
    --import-metadata <meta.json>   Import and use metadata from file
    --datefmt                       Change the default format for all date-times
        (default: %Y-%m-%dT%H:%M:%S%.3f%z)

EXAMPLES:

# Just dump eventlogs as they arrive on localhost, in JSON
    .\evtq.exe

# List all sessions ever opened in a backed-up Security eventlog, in JSON
    .\evtq.exe --from-backup .\security.evtx -i Security/Microsoft-Windows-Security-Auditing/4624

# Dump all events that ever happened except one type, from a remote host, in CSV
    .\evtq.exe --from-host server1.lab --dump-existing -e Application/*/1026 --to-csv .\a.csv

# List processes as they are created on a remote host using explicit credentials
    .\evtq.exe --from-host lab1/Admin:MyPassw0rd@server1.lab --to-json .\procs.json -i */*/4688

# Dump events as they happen on localhost, in CSV format, removing columns you don't use
    .\evtq.exe --to-csv .\all.csv -O timestamp,provider,eventid,version,variant1,...,variant15
        "#)
        .arg(Arg::with_name("verbosity")
            .short("v")
            .long("verbose")
            .multiple(true))
        .arg(Arg::with_name("version")
            .short("V")
            .long("version"))
        .arg(Arg::with_name("export-metadata")
            .long("export-metadata")
            .value_name("meta.json")
            .default_value("stdout")
            .help(""))
        .arg(Arg::with_name("import-metadata")
            .long("import-metadata")
            .value_name("meta.json"))
        .arg(Arg::with_name("from-host")
            .long("from-host")
            .default_value("localhost"))
        .arg(Arg::with_name("from-backup")
            .long("from-backup")
            .takes_value(true))
        .arg(Arg::with_name("to-json")
            .long("to-json")
            .default_value("stdout"))
        .arg(Arg::with_name("to-xml")
            .long("to-xml")
            .default_value("stdout"))
        .arg(Arg::with_name("to-csv")
            .long("to-csv")
            .default_value("stdout"))
        .arg(Arg::with_name("to-tsv")
            .long("to-tsv")
            .default_value("stdout"))
        .arg(Arg::with_name("append")
            .long("append")
            .short("a"))
        .arg(Arg::with_name("dump-existing")
            .long("dump-existing"))
        .arg(Arg::with_name("no-wait")
            .long("no-wait"))
        .arg(Arg::with_name("datefmt")
            .long("datefmt")
            .default_value("%Y-%m-%dT%H:%M:%S%.3f%z"))
        .arg(Arg::with_name("json-pretty")
            .long("json-pretty"))
        .arg(Arg::with_name("no-system-metadata")
            .long("no-system-metadata"))
        .arg(Arg::with_name("list-channels")
            .long("list-channels"))
        .arg(Arg::with_name("include")
            .short("i")
            .long("include")
            .takes_value(true)
            .multiple(true)
            .default_value("*/*/*/*"))
        .arg(Arg::with_name("exclude")
            .short("e")
            .long("exclude")
            .takes_value(true)
            .multiple(true))
        /* TODO: support raw XPath queries for advanced users
        .arg(Arg::with_name("raw-include")
            .long("raw-include")
            .takes_value(true)
            .multiple(true)
            .help("Only render events matching the given XML XPath query qualifier"))
        .arg(Arg::with_name("raw-exclude")
            .long("raw-exclude")
            .takes_value(true)
            .multiple(true)
            .help("Don't render events matching the given XML XPath query qualifier")) */
        .arg(Arg::with_name("columns")
            .short("O")
            .long("columns")
            .takes_value(true)
            .default_value("hostname,recordid,timestamp,provider,eventid,version,level_name,task_name,keyword_names,formatted_message,variant1,...,variant15"))
        //.group(ArgGroup::with_name("source").args(&["from-host", "from-backup"]))
        //TODO: ArgGroups with mutual exclusion
        .get_matches();

    set_log_level(args.occurrences_of("verbosity") as u8);

    let mut render_cfg = RenderingConfig {
        render_callback: render_event_json,
        output_file: Box::new(Mutex::new(std::io::stdout())),
        datefmt: "".to_string(),
        metadata: BTreeMap::new(),
        field_separator: '\0',
        json_pretty: false,
        columns: vec![],
        rendering_start: std::time::Instant::now(),
        event_counter: AtomicU64::new(0),
    };

    let list_channels = args.occurrences_of("list-channels") != 0;
    let do_import_system_fields = args.occurrences_of("no-system-metadata") == 0 && !list_channels;
    render_cfg.datefmt = args.value_of("datefmt").unwrap().to_owned();
    render_cfg.columns = parse_column_names(args.value_of("columns").unwrap())?;
    render_cfg.json_pretty = args.occurrences_of("json-pretty") > 0;

    let append = args.occurrences_of("append") > 0;
    let dump_existing = args.occurrences_of("dump-existing") > 0;
    let tail_follow = args.occurrences_of("no-wait") == 0;
    let mut system_field_defs_read = false;
    let include: Vec<&str> = args.values_of("include").unwrap().collect();
    let exclude: Vec<&str> = if args.occurrences_of("exclude") > 0 {
        args.values_of("exclude").unwrap().collect()
    } else {
        vec![]
    };

    if args.occurrences_of("import-metadata") == 1 {
        let in_path = args.value_of("import-metadata").unwrap();
        let mut in_file = match OpenOptions::new().read(true).open(in_path) {
            Err(e) => return Err(format !("Could not open file {} : {}", in_path, e)),
            Ok(f) => f,
        };

        if do_import_system_fields && !system_field_defs_read {
            match import_metadata_from_system() {
                Ok(system_metadata) => update_metadata_with(&mut render_cfg.metadata, &system_metadata),
                Err(e) => warn!("Could not import system metadata: only using the given export ({})", e),
            }
            system_field_defs_read = true;
        }
        let imported_field_defs = import_metadata_from_file(&mut in_file)?;
        update_metadata_with(&mut render_cfg.metadata, &imported_field_defs);
    }

    if args.occurrences_of("export-metadata") == 1 {
        let out_path = args.value_of("export-metadata").unwrap();
        let mut out_file : Box<dyn std::io::Write> = if out_path.eq("stdout") {
            Box::from(io::stdout())
        } else {
            match OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(out_path) {
                Err(e) => return Err(format!("Could not open file {} : {}", out_path, e)),
                Ok(f) => Box::from(f),
            }
        };
        if do_import_system_fields && !system_field_defs_read {
            match import_metadata_from_system() {
                Ok(system_field_defs) => update_metadata_with(&mut render_cfg.metadata, &system_field_defs),
                Err(e) => warn!("Some fields will be left unnamed: unable to read metadata from system, {}", e),
            }
            system_field_defs_read = true;
        }
        return export_metadata_to_file(&render_cfg.metadata, &mut out_file, render_cfg.json_pretty);
    }

    if args.occurrences_of("to-xml") == 1 {
        let out_path = args.value_of("to-xml").unwrap();
        let out_file = if out_path.eq("stdout") {
            Box::from(io::stdout()) as Box<dyn std::io::Write>
        } else {
            match OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(out_path) {
                Err(e) => return Err(format!("Could not open file {} : {}", out_path, e)),
                Ok(f) => Box::from(f) as Box<dyn std::io::Write>,
            }
        };
        render_cfg.render_callback = render_event_xml;
        render_cfg.output_file = Box::from(Mutex::new(out_file));
    }
    else if args.occurrences_of("to-csv") == 1 {
        let out_path = args.value_of("to-csv").unwrap();
        let out_file = if out_path.eq("stdout") {
            Box::from(io::stdout()) as Box<dyn std::io::Write>
        } else {
            match OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(out_path) {
                Err(e) => return Err(format!("Could not open file {} : {}", out_path, e)),
                Ok(f) => Box::from(f) as Box<dyn std::io::Write>,
            }
        };
        render_cfg.render_callback = render_event_csv;
        render_cfg.output_file = Box::from(Mutex::new(out_file));
        render_cfg.field_separator = ',';
    }
    else if args.occurrences_of("to-tsv") == 1 {
        let out_path = args.value_of("to-tsv").unwrap();
        let out_file = if out_path.eq("stdout") {
            Box::from(io::stdout()) as Box<dyn std::io::Write>
        } else {
            match OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(out_path) {
                Err(e) => return Err(format!("Could not open file {} : {}", out_path, e)),
                Ok(f) => Box::from(f) as Box<dyn std::io::Write>,
            }
        };
        render_cfg.render_callback = render_event_csv;
        render_cfg.output_file = Box::from(Mutex::new(out_file));
        render_cfg.field_separator = '\t';
    }
    else {
        let out_path = args.value_of("to-json").unwrap();
        let out_file = if out_path.eq("stdout") {
            Box::from(io::stdout()) as Box<dyn std::io::Write>
        } else {
            match OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(out_path) {
                Err(e) => return Err(format !("Could not open file {} : {}", out_path, e)),
                Ok(f) => Box::from(f) as Box<dyn std::io::Write>,
            }
        };
        if do_import_system_fields && !system_field_defs_read {
            match import_metadata_from_system() {
                Ok(system_field_defs) => update_metadata_with(&mut render_cfg.metadata, &system_field_defs),
                Err(e) => warn!("JSON output will have generic field names: unable to read event definitions from system, {}", e),
            }
            system_field_defs_read = true;
        }
        render_cfg.render_callback = render_event_json;
        render_cfg.output_file = Box::from(Mutex::new(out_file));
    }
    info!("Imported metadata from {} providers", render_cfg.metadata.len());

    if args.occurrences_of("from-backup") == 1 {
        let path = args.value_of("from-backup").unwrap();
        let xml_filter = xml_query_from_filters(&include, &exclude, None)?;
        let xml_filter = xml_filter.get("*").unwrap_or(&None);
        verbose!("Opening file {}...", path);
        let session = windows::open_evt_backup(path, xml_filter)?;

        info!("Starting event rendering loop");
        windows::synchronous_poll_all_events(&session, &render_cfg)?;
        info!("Done");
    }
    else {
        let uri = args.value_of("from-host").unwrap();
        let parts : Vec<&str> = uri.rsplitn(2,"@").collect();
        let hostname = *parts.get(0).unwrap();
        let rpc_creds;
        let rpc_creds = if parts.len() == 1 {
            info!("Authenticating to {} with implicit credentials...", hostname);
            None
        }
        else {
            let parts : Vec<&str> = parts[1].splitn(2, r"/").collect();
            let (domain, parts) = if parts.len() != 2 {
                (".", parts[0].splitn(2, ":").collect::<Vec<&str>>())
            } else {
                (parts[0], parts[1].splitn(2, ":").collect::<Vec<&str>>())
            };
            if parts.len() != 2 {
                return Err(format!("Unable to parse username:password from '{}'", uri));
            }
            let (username, password) = (parts[0], parts[1]);
            rpc_creds = RpcCredentials { domain, username, password };
            info!("Authenticating to {} as {}\\{}", hostname, domain, username);
            Some(&rpc_creds)
        };

        let session = windows::open_evt_session(hostname, rpc_creds)?;
        info!("Authenticated to host");

        let mut channels = Vec::new();
        for channel_name in windows::evt_list_channels(&session)? {
            if !windows::can_channel_be_subscribed(&session, &channel_name)? {
                continue;
            }
            channels.push(channel_name);
        }
        if args.occurrences_of("list-channels") != 0 {
            for channel_name in &channels {
                println!("{}", channel_name);
            }
            return Ok(());
        }
        info!("Found {} channels which can be subscribed to. Subscribing...", channels.len());

        let xml_filters = xml_query_from_filters(&include, &exclude, Some(&channels))?;
        // Ensure the RenderingConfig is never freed. This is the price to pay to use the
        // asynchronous subscription API. All this just because the synchronous API developer
        // was too lazy to make a heap allocation, and had to allocate a hardcoded array of 256 (?)
        // handles on the stack...
        let mut subscriptions = Vec::new();
        for (channel_name, xml_filter) in xml_filters {
            verbose!("Subscribing to channel {}", channel_name);
            match xml_filter {
                Some(ref xml) => debug!("Using XML filter:\n{}", xml),
                None => debug!("(without any filter set up)"),
            }
            let h_subscription = windows::subscribe_channel(&session, &channel_name, &render_cfg, &xml_filter, dump_existing)?;
            subscriptions.push(h_subscription);
        }
        info!("Starting event rendering loop");
        let mut last_event_count = 0;
        while subscriptions.len() > 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let current_event_count = render_cfg.event_counter.load(Relaxed);
            if current_event_count == last_event_count {
                if tail_follow {
                    debug!("Waiting for more events");
                }
                else {
                    info!("Done");
                    break;
                }
            }
            last_event_count = current_event_count;
        }
        // Close all handles when all events have been received, not before
        info!("Done. Cleaning up all channel subscriptions...");
        std::mem::drop(subscriptions);
    }

    Ok(())
}
