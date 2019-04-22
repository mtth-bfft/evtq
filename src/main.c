#include <Windows.h>
#include <tchar.h>
#include <stdio.h>
#include "main.h"
#include "mem.h"
#include "metadata.h"
#include "render.h"
#include "inputs/live.h"
#include "inputs/backup.h"
#include "outputs/tsv.h"
#include "outputs/xml.h"
#include "outputs/json.h"

BOOL bAppend = FALSE;
BOOL bEver = FALSE;
BOOL bGzip = FALSE;
long eventCount = 0;
int verbosity = 0;
input_t input = INPUT_DEFAULT;
output_t output = OUTPUT_DEFAULT;
FILE *fOutput = NULL;

static void print_usage()
{
    _ftprintf(stderr, TEXT("Usage: evtq [input] [output] [options]\n"));
    _ftprintf(stderr, TEXT("\n"));
    _ftprintf(stderr, TEXT("Input : default is all local eventlogs\n"));
    _ftprintf(stderr, TEXT("  --from-host [[domain/]username[:password]@]<hostname>\n"));
    _ftprintf(stderr, TEXT("  --from-evtx <filename>.evtx\n"));
    _ftprintf(stderr, TEXT("  --from-evt  <filename>.evt\n"));
    _ftprintf(stderr, TEXT("\n"));
    _ftprintf(stderr, TEXT("Output: default is to print on screen as JSON\n"));
    _ftprintf(stderr, TEXT("  --to-tsv  [filename]\n"));
    _ftprintf(stderr, TEXT("  --to-csv  [filename]\n"));
    _ftprintf(stderr, TEXT("  --to-xml  [filename]\n"));
    _ftprintf(stderr, TEXT("  --to-json [filename]\n"));
    _ftprintf(stderr, TEXT("\n"));
    _ftprintf(stderr, TEXT("Options:\n"));
    _ftprintf(stderr, TEXT("  -h --help              display this help text\n"));
    _ftprintf(stderr, TEXT("  -v --verbose           increase verbosity(can be repeated)\n"));
    _ftprintf(stderr, TEXT("  -a --append            append to output files, don't truncate\n"));
    _ftprintf(stderr, TEXT("  -e --ever              for live inputs, dump existing events instead of new ones\n"));
    _ftprintf(stderr, TEXT("  -i --import-providers <list.json>   JSON file with known events and field names\n"));
    _ftprintf(stderr, TEXT("  -e --export-providers <list.json>   write the host's registered publishers to disk\n"));
    _ftprintf(stderr, TEXT("  -z --gzip              compress output with gzip\n"));
    _ftprintf(stderr, TEXT("  -f --filter[!][channel]/[provider]/[eventID]/[version]\n"));
    _ftprintf(stderr, TEXT("         only show events matching(or not matching, if prefixed with !)\n"));
    _ftprintf(stderr, TEXT("         (use * as wildcards) (can be repeated)\n"));
    _ftprintf(stderr, TEXT("  -s --stats             display statistics about event counts at the end\n"));
    _ftprintf(stderr, TEXT("  -n <number>            only output N events\n"));
    _ftprintf(stderr, TEXT("\n"));
}

int render_event_callback(EVT_HANDLE hEvent)
{
   int res = 0;
   EVT_HANDLE hContextSystem = NULL;
   DWORD dwBufferSize = 0;
   DWORD dwPropertyCount = 0;
   PEVT_VARIANT pSysProps = NULL;

   // First, extract common "system" properties for statistics and filtering
   hContextSystem = EvtCreateRenderContext(0, NULL, EvtRenderContextSystem);
   if (hContextSystem == NULL)
   {
       res = GetLastError();
       _ftprintf(stderr, TEXT("Error: unable to create system rendering context, code %u\n"), res);
       goto cleanup;
   }
   if (EvtRender(hContextSystem, hEvent, EvtRenderEventValues, 0, NULL, &dwBufferSize, &dwPropertyCount) ||
       GetLastError() != ERROR_INSUFFICIENT_BUFFER)
   {
       res = GetLastError();
       _ftprintf(stderr, TEXT("Error: unable to render event system values, code %u\n"), res);
       goto cleanup;
   }
   pSysProps = (PEVT_VARIANT)safe_alloc(dwBufferSize);
   if (!EvtRender(hContextSystem, hEvent, EvtRenderEventValues, dwBufferSize, pSysProps, &dwBufferSize, &dwPropertyCount))
   {
       res = GetLastError();
       _ftprintf(stderr, TEXT("Error: unable to render event system values, code %u\n"), res);
       goto cleanup;
   }

   if (output == OUTPUT_TSV)
       res = render_event_tsv(fOutput, hEvent);
   else if (output == OUTPUT_XML)
       res = render_event_xml(fOutput, hEvent);
   else if (output == OUTPUT_JSON)
       res = render_event_json(fOutput, hEvent);

cleanup:
   if (pSysProps != NULL)
       safe_free(pSysProps);
   return res;
}

int _tmain(int argc, TCHAR* argv[])
{
	int res = 0;
    BOOL bExportAction = FALSE;
	PWSTR swzInputPath = NULL;
	PWSTR swzOutputPath = NULL;

   init_render_output();

   for (int i = 1; i < argc; i++)
   {
      TCHAR *arg = argv[i];
      if (_tcsicmp(arg, TEXT("-h")) == 0 || _tcsicmp(arg, TEXT("--help")) == 0)
      {
         print_usage();
         return 1;
      }
      else if (_tcsicmp(arg, TEXT("-f")) == 0 || _tcsnicmp(arg, TEXT("--filter"), 8) == 0)
      {
         PWSTR swzFilter = (arg[8] == TEXT('=')) ? &(arg[9]) : argv[++i];
         //TODO: register filter in hash table ()
         (void)(swzFilter);
      }
      else if (_tcsicmp(arg, TEXT("-a")) == 0 || _tcsicmp(arg, TEXT("--append")) == 0)
      {
         bAppend = TRUE;
      }
      else if (_tcsicmp(arg, TEXT("-e")) == 0 || _tcsicmp(arg, TEXT("--ever")) == 0)
      {
         bEver = TRUE;
      }
      else if (_tcsicmp(arg, TEXT("-z")) == 0 || _tcsicmp(arg, TEXT("--gzip")) == 0)
      {
         bGzip = TRUE;
      }
      else if (_tcsicmp(arg, TEXT("-v")) == 0 || _tcsicmp(arg, TEXT("--verbose")) == 0)
      {
         verbosity++;
      }
      else if (_tcsicmp(arg, TEXT("-n")) == 0)
      {
         PWSTR swzNum = argv[++i];
         if (swzNum == NULL || (eventCount = _tcstol(swzNum, NULL, 10)) <= 0)
         {
            _ftprintf(stderr, TEXT("Error: an integer is required after -n\n"));
            print_usage();
            return 1;
         }
      }
      else if (_tcsnicmp(arg, TEXT("--import-publishers"), 19) == 0)
      {
         PCTSTR swzPublishersPath = (arg[19] == TEXT('=')) ? &(arg[20]) : argv[++i];
         init_fieldnames_from_system(); // force an import from the current system
         // because if we do it after the call to init_fieldnames_from_export(), we'll overwrite user-provided data
         res = init_fieldnames_from_export(swzPublishersPath);
         if (res != 0)
            return res;
      }
      else if (_tcsnicmp(arg, TEXT("--export-publishers"), 19) == 0)
      {
          PCTSTR swzPublishersPath = (arg[19] == TEXT('=')) ? &(arg[20]) : argv[++i];
          init_fieldnames_from_system(); // force an import from the current system
          // because otherwise there might not be anything to export
          res = export_fieldnames_to_file(swzPublishersPath);
          if (res != 0)
              return res;
          bExportAction = TRUE;
      }
      else if (_tcsnicmp(arg, TEXT("--from-host"), 11) == 0)
      {
         if (input != INPUT_DEFAULT)
         {
            _ftprintf(stderr, TEXT("Error: cannot specify multiple inputs\n"));
            print_usage();
            return 1;
         }
         input = INPUT_REMOTEHOST;
         if (arg[11] == TEXT('='))
         {
             swzInputPath = &(arg[12]);
         }
         else if (i == argc - 1)
         {
             _ftprintf(stderr, TEXT("Error: option --from-host requires an argument\n"));
             print_usage();
             return 1;
         }
         swzInputPath = argv[++i];
      }
      else if (_tcsnicmp(arg, TEXT("--from-evtx"), 11) == 0)
      {
         if (input != INPUT_DEFAULT)
         {
            _ftprintf(stderr, TEXT("Error: cannot specify multiple inputs\n"));
            print_usage();
            return 1;
         }
         input = INPUT_EVTX;
         if (arg[11] == TEXT('='))
         {
             swzInputPath = &(arg[12]);
         }
         else if (i == argc - 1)
         {
             _ftprintf(stderr, TEXT("Error: option --from-evtx requires an argument\n"));
             print_usage();
             return 1;
         }
         swzInputPath = argv[++i];
      }
      else if (_tcsnicmp(arg, TEXT("--from-evt"), 10) == 0)
      {
         if (input != INPUT_DEFAULT)
         {
            _ftprintf(stderr, TEXT("Error: cannot specify multiple inputs\n"));
            print_usage();
            return 1;
         }
         input = INPUT_EVT;
         if (arg[10] == TEXT('='))
         {
             swzInputPath = &(arg[11]);
         }
         else if (i == argc - 1)
         {
             _ftprintf(stderr, TEXT("Error: option --from-evt requires an argument\n"));
             print_usage();
             return 1;
         }
         swzInputPath = argv[++i];
      }
      else if (_tcsnicmp(arg, TEXT("--to-tsv"), 8) == 0)
      {
         if (output != OUTPUT_DEFAULT)
         {
            _ftprintf(stderr, TEXT("Error: cannot specify multiple outputs\n"));
            print_usage();
            return 1;
         }
         output = OUTPUT_TSV;
         swzOutputPath = (arg[8] == TEXT('=')) ? &(arg[9]) : (argv[i + 1] == NULL || argv[i + 1][0] == TEXT('-') ? NULL : argv[++i]);
      }
      else if (_tcsnicmp(arg, TEXT("--to-xml"), 8) == 0)
      {
          if (output != OUTPUT_DEFAULT)
          {
              _ftprintf(stderr, TEXT("Error: cannot specify multiple outputs\n"));
              print_usage();
              return 1;
          }
          output = OUTPUT_XML;
          swzOutputPath = (arg[8] == TEXT('=')) ? &(arg[9]) : (argv[i + 1] == NULL || argv[i + 1][0] == TEXT('-') ? NULL : argv[++i]);
      }
      else if (_tcsnicmp(arg, TEXT("--to-json"), 9) == 0)
      {
          if (output != OUTPUT_DEFAULT)
          {
              _ftprintf(stderr, TEXT("Error: cannot specify multiple outputs\n"));
              print_usage();
              return 1;
          }
          output = OUTPUT_JSON;
          swzOutputPath = (arg[9] == TEXT('=')) ? &(arg[10]) : (argv[i + 1] == NULL || argv[i + 1][0] == TEXT('-') ? NULL : argv[++i]);
          init_fieldnames_from_system();
      }
      else
      {
         _ftprintf(stderr, TEXT("Error: unknown option '%s'\n"), arg);
         print_usage();
         return 1;
      }
   }

   // Exit after exporting, if no other options were passed
   if (input == INPUT_DEFAULT && output == OUTPUT_DEFAULT && bExportAction)
       return 0;

   // Apply default values
   if (input == INPUT_DEFAULT)
      input = INPUT_LOCALHOST;
   if (output == OUTPUT_DEFAULT)
      output = OUTPUT_JSON;

   // Only load metadata if it is useful with the selected output (JSON only)
   if (output == OUTPUT_JSON)
       init_fieldnames_from_system();

   // Create (or open in append mode) the output file
   if (swzOutputPath != NULL)
   {
      errno_t err = _wfopen_s(&fOutput, swzOutputPath, (bAppend ? L"a" : L"w"));
      if (err != 0 || fOutput == NULL)
      {
         res = errno;
         _ftprintf(stderr, TEXT("Error: unable to open output file '%s', error code %u\n"), swzOutputPath, res);
         return res;
      }
   }
   else
   {
       fOutput = stdout;
   }

   // Read from the selected input (rendering on the selected output is done by render_event_callback())
   if (input == INPUT_LOCALHOST)
   {
       res = open_source_live(NULL, NULL, NULL, NULL, !bEver);
   }
   else if (input == INPUT_EVT || input == INPUT_EVTX)
   {
       res = open_source_backup(swzInputPath);
   }

	return res;
}