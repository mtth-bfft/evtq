# evtq

Windows eventlog query, parsing, and formatting tool. Supports querying live hosts (localhost or remote) via MSRPC, and backup log files in .evt and .evtx formats. Provides JSON, CSV, TSV, XML outputs, and filtering based on Channel, Provider name, EventID, and version. Examples are included below.

## Usage

```
    evtq.exe [input] [filtering] [output] [common]

COMMON:
 -h --help                             Display this help text
 -V --version                          Display the current version
 -v --verbose                          Increase verbosity (can be repeated for more information)
 -O --columns                          Comma-separated list of columns to include in JSON, CSV, and TSV outputs
        (default: hostname,recordid,timestamp,provider,eventid,version,variant1,...,variant15)
    --datefmt                          Change the default format for all date-times
        (default: %Y-%m-%dT%H:%M:%S%.3f%z)

FIELD NAMING: (only used with the JSON output)
    --no-system-fields                 Don't load event field definitions from the live OS
    --export-event-fields <bak.json>   Export event field definitions to re-import them on another host
    --import-event-fields <bak.json>   Import event field definitions exported from another host

INPUTS:
    --from-backup <filename.evt(x)>    Read events from a backup .evtx or .evt
    --from-host <URI>                  Read events as they happen on a live host via RPC (default: localhost)
        URI format: <[[domain/]username:password@]hostname>
    --dump-existing                    Also process existing (past) events from the queried host
    --no-wait                          Don't wait for future events to arrive from the queried host
    --list-channels                    Don't dump events, just list available channels from the host

OUTPUTS:
    --to-xml  [output.xml]             Render events as lines of raw XML (default: stdout)
    --to-csv  [output.csv]             Render events as lines of comma-separated columns (default: stdout)
    --to-tsv  [output.tsv]             Render events as lines of tab-separated columns (default: stdout)
    --to-json [output.json]            Render events to a JSON file with field names (default: stdout)
    --json-pretty                      Add spaces and line feeds to make the JSON human-readable
 -a --append                           Don't overwrite output files if they exist

FILTERING:
 -i --include <filter>                 Only render events matching this filter (default: */*/*/*)
 -e --exclude <filter>                 Don't render events matching this filter
      Filter format: ChannelName/ProviderName/EventID/Version
      Each field can be replaced with a * as a wildcard
```

## Examples

- Just dump eventlogs as they arrive on localhost, in JSON

```
    .\evtq.exe
```

- List all sessions ever opened in a backed-up Security eventlog, in JSON

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

To allow remote hosts to use the EventLogs RPC endpoint, your host must be running Windows Vista or later, and you must enable the "Remote Event Log Management" exception in Windows Firewall.

## TODO

- See OpenBackupEventLog()
- Implement formatting for arrays
- GZIP compression

