#include <tchar.h>
#include <stdio.h>
#include "mem.h"
#include "sources/live.h"
#include "sources/backup.h"

void print_usage()
{
	_ftprintf(stderr, TEXT("Usage: TODO\n"));
}

int _tmain(int argc, TCHAR* argv[])
{
	int res = 0;
	BOOL bAppend = FALSE;
	BOOL bFollow = FALSE;
	BOOL bGzip = FALSE;
	int verbose = 0;
	long evtCount = 0;
	PWSTR swzProvidersPath = NULL;
	PWSTR swzFieldMapPath = NULL;
	PWSTR swzInputPath = NULL;
	PWSTR swzOutputPath = NULL;

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
		else if (_tcsicmp(arg, TEXT("-w")) == 0 || _tcsicmp(arg, TEXT("--follow")) == 0)
		{
			bFollow = TRUE;
		}
		else if (_tcsicmp(arg, TEXT("-z")) == 0 || _tcsicmp(arg, TEXT("--gzip")) == 0)
		{
			bGzip = TRUE;
		}
		else if (_tcsicmp(arg, TEXT("-v")) == 0 || _tcsicmp(arg, TEXT("--verbose")) == 0)
		{
			verbose++;
		}
		else if (_tcsicmp(arg, TEXT("-n")) == 0)
		{
			PWSTR swzNum = argv[++i];
			if (swzNum == NULL || (evtCount = _tcstol(swzNum, NULL, 10)) <= 0)
			{
				_ftprintf(stderr, TEXT("Error: an integer is required after -n\n"));
				print_usage();
				return 1;
			}
		}
		else if (_tcsnicmp(arg, TEXT("--providers"), 11) == 0)
		{
			if (swzProvidersPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple provider definition files\n"));
				print_usage();
				return 1;
			}
			swzProvidersPath = (arg[11] == TEXT('=')) ? &(arg[11]) : argv[++i];
			res = map_file_readonly(swzProvidersPath, &swzProviders, &dwProvidersLen);

		}
		else if (_tcsnicmp(arg, TEXT("--host"), 6) == 0)
		{
			if (swzInputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sources\n"));
				print_usage();
				return 1;
			}
			swzInputPath = (arg[6] == TEXT('=')) ? &(arg[7]) : argv[++i];
		}
		else if (_tcsnicmp(arg, TEXT("--evtx"), 6) == 0)
		{
			if (swzInputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sources\n"));
				print_usage();
				return 1;
			}
			swzInputPath = (arg[6] == TEXT('=')) ? &(arg[7]) : argv[++i];
		}
		else if (_tcsnicmp(arg, TEXT("--tsv"), 5) == 0)
		{
			if (swzInputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sources\n"));
				print_usage();
				return 1;
			}
			swzInputPath = (arg[5] == TEXT('=')) ? &(arg[6]) : argv[++i];
		}
		else if (_tcsnicmp(arg, TEXT("--csv"), 5) == 0)
		{
			if (swzInputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sources\n"));
				print_usage();
				return 1;
			}
			swzInputPath = (arg[5] == TEXT('=')) ? &(arg[6]) : argv[++i];
		}
		else if (_tcsnicmp(arg, TEXT("--to-csv"), 8) == 0)
		{
			if (swzOutputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sinks\n"));
				print_usage();
				return 1;
			}
			swzOutputPath = (arg[8] == TEXT('=')) ? &(arg[9]) : (argv[i+1] == NULL || argv[i+1][0] == TEXT('-') ? NULL : argv[++i]);
			swzFieldMapPath = _tcsstr(swzOutputPath, TEXT(","));
			if (swzFieldMapPath == NULL)
			{
				_ftprintf(stderr, TEXT("Error: CSV output requires both <output file>,<field map> paths\n"));
				print_usage();
				return 1;
			}
			*swzFieldMapPath = TEXT('\0');
			swzFieldMapPath++;
		}
		else if (_tcsnicmp(arg, TEXT("--to-tsv"), 8) == 0)
		{
			if (swzOutputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sinks\n"));
				print_usage();
				return 1;
			}
			swzOutputPath = (arg[8] == TEXT('=')) ? &(arg[9]) : (argv[i + 1] == NULL || argv[i + 1][0] == TEXT('-') ? NULL : argv[++i]);
			swzFieldMapPath = _tcsstr(swzOutputPath, TEXT(","));
			if (swzFieldMapPath == NULL)
			{
				_ftprintf(stderr, TEXT("Error: TSV output requires both <output file>,<field map> paths\n"));
				print_usage();
				return 1;
			}
			*swzFieldMapPath = TEXT('\0');
			swzFieldMapPath++;
		}
		else if (_tcsnicmp(arg, TEXT("--to-xml"), 8) == 0)
		{
			if (swzOutputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sinks\n"));
				print_usage();
				return 1;
			}
			swzOutputPath = (arg[8] == TEXT('=')) ? &(arg[9]) : (argv[i + 1] == NULL || argv[i + 1][0] == TEXT('-') ? NULL : argv[++i]);
		}
		else if (_tcsnicmp(arg, TEXT("--to-json"), 8) == 0)
		{
			if (swzOutputPath != NULL)
			{
				_ftprintf(stderr, TEXT("Error: cannot specify multiple sinks\n"));
				print_usage();
				return 1;
			}
			swzOutputPath = (arg[9] == TEXT('=')) ? &(arg[10]) : (argv[i + 1] == NULL || argv[i + 1][0] == TEXT('-') ? NULL : argv[++i]);
		}
		else
		{
			_ftprintf(stderr, TEXT("Error: unknown option '%s'\n"), arg);
			print_usage();
			return 1;
		}
	}



	printf(" Input: %ws\n", swzInputPath);
	printf(" Output: %ws\n", swzOutputPath);

	//int res = open_source_backup(L"C:\\Users\\User\\Desktop\\application.evtx");
	//int res = open_source_live(NULL, NULL, NULL, NULL, TRUE);
	if (res != 0)
		goto cleanup;
	
cleanup:
	return res;
}