# evtq

## Usage

```
    evtq [input] [output] [options]
    Input : default is to query all local eventlogs
      --from-host [[domain/]username:password@]<hostname>
      --from-evtx <filename>.evtx
      --from-evt  <filename>.evt
    Output: default is to print on screen as JSON
      --to-tsv  [filename]
      --to-csv  [filename]
      --to-xml  [filename]
      --to-json [filename]
    Options:
      -h --help                       display this help text
      -V --version                    display the current version and exit
      -v --verbose                    increase verbosity (can be repeated)
      -a --append                     append to output files, don't truncate
      -e --ever                       for live inputs, dump existing events instead of new ones
      -i --import-providers <x.json>  JSON file with known events and field names
      -e --export-providers <x.json>  write the host's registered publishers to disk
      -s --stats                      display statistics about event counts at the end
      -n --only <number>              stop after writing a given number of events
    [work in progress features:]
      -z --gzip                       compress output with gzip
      -f --filter [!][channel]/[provider]/[eventID]/[version]
             only show events matching (or not matching, if prefixed with !)
             (use * as wildcards) (can be repeated)
```

To allow remote hosts to use the EventLogs RPC endpoint, your host must be running Windows Vista or later, and you must enable the "Remote Event Log Management" exception in Windows Firewall.

## Compiling

To compile this project you need Visual Studio. You might want to replace the .lib files in ./lib/, but everything should compile out of the box.

# TODO

- Check out the OpenBackupEventLog() old API
- GZIP compression
- Filtering
