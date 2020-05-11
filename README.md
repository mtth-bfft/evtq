# evtq

Windows eventlog querying/parsing/formatting tool.

Supports querying live hosts (localhost or remote) via MSRPC, and backup log files in .evt and .evtx formats. Provides JSON, CSV, TSV, XML outputs, and filtering based on Channel, Provider name, EventID, and version.

Field names and message templates are enriched from the host's event providers' metadata (or any copy of another host's metadata).

Examples are included below.

## Usage

```
evtq.exe [input] [filtering] [output] [common]

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
    --to-json [output.json]         Render events as lines of JSON objects (default: stdout)
    --to-xml  [output.xml]          Render events as lines of unmodified event XML (default: stdout)
    --to-csv  [output.csv]          Render events as lines of comma-separated columns (default: stdout)
    --to-tsv  [output.tsv]          Render events as lines of tab-separated columns (default: stdout)
    --json-pretty                   Add spaces and line feeds to JSON outputs
 -a --append                        Don't overwrite output files if they exist

COMMON:
 -h --help                          Display this help text
 -V --version                       Display the current version
 -v --verbose                       Increase verbosity (can be repeated for extra information)
 -O --columns                       Comma-separated list of columns to output in JSON, CSV, or TSV
      (default: hostname,recordid,timestamp,provider,eventid,version,formatted_message,variant1,...,variant15)
      (use 'unformatted_message' or remove 'formatted_message' if you don't want to duplicate
       information with the variantN fields, or if you only care about individual fields)
    --no-system-metadata            Don't load field names, types, and message strings from the live OS
    --export-metadata <meta.json>   Export metadata to file
    --import-metadata <meta.json>   Import and use metadata from file
    --datefmt                       Change the default format for all date-times
        (default: %Y-%m-%dT%H:%M:%S%.3f%z)
```

## Examples

- Just dump live eventlogs as they are generated on localhost, in JSON (the default output format, which enriches event fields with their name and correct type)

```
    .\evtq.exe
    {
      "hostname": "DESKTOP-DGDV3HL",
      "recordid": 36548,
      "timestamp": "2020-05-11T01:14:33.442+0000",
      "provider": "Microsoft-Windows-Security-Auditing",
      "eventid": 5379,
      "version": 0,
      "message": "Credential Manager credentials were read."
      "SubjectUserSid": "S-1-5-21-2660493220-2051396753-1551960823-1001",
      "SubjectUserName": "User",
      "SubjectDomainName": "DESKTOP-DGDV3HL",
      "SubjectLogonId": 159265,
      "TargetName": "",
      "Type": 0,
      "CountOfCredentialsReturned": 2,
      "ReadOperation": "%%8100",
      "ReturnCode": 0,
      "ProcessCreationTime": "2020-05-11T01:14:33.318+0000",
      "ClientProcessId": 11932
    }
```

- List all sessions ever opened in a backed-up Security eventlog

```
    .\evtq.exe --from-backup .\security.evtx -i Security/Microsoft-Windows-Security-Auditing/4624
```

- Dump all events that ever happened except one type, from a remote host, in CSV

```
    .\evtq.exe --from-host server1.lab.local --dump-existing -e Application/*/1026 --to-csv .\a.csv
```

- List processes as they are created on a remote host using explicit credentials

```
    .\evtq.exe --from-host lab1/Admin:MyPassw0rd@server1.lab.local --to-json .\procs.json -i */*/4688
```

- Dump events as they happen on localhost, in CSV format, removing columns you don't use

```
    .\evtq.exe --to-csv .\all.csv -O timestamp,provider,eventid,version,variant1,...,variant15
```

- Show events as they arrive on a remote host, using the published listing of event definitions instead of the system's one:

```
    .\evtq.exe --from-host server1.lab.local --no-system-fields --import-event-fields .\event_definitions.json
```

To allow remote hosts to use the EventLogs RPC endpoint, your host must be running Windows Vista or later, and you must enable the "Remote Event Log Management" exception in Windows Firewall.

## Contributing

If you encounter any issue using this tool, or would like to see new features implemented, open an issue.

Also, the `event_definitions.json` listing is a constant work in progress which needs to be updated and extended with new event definitions you find that might be of interest to the community.
To generate a similar JSON export, on the host with event definitions, run: `evtq.exe --export-metadata .\event_definitions.json --json-pretty`

## TODO

- See OpenBackupEventLog()
- Implement formatting for arrays
- Add support for raw XML XPath queries
- GZIP compression
