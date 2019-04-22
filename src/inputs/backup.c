#include <Windows.h>
#include <tchar.h>
#include <winevt.h>
#include <stdio.h>
#include "inputs/backup.h"
#include "main.h"

int open_source_backup(PCTSTR swzAbsolutePath)
{
	int res = 0;
	HANDLE hFeed = NULL;
	EVT_HANDLE hEvent = NULL;
	DWORD dwEvtCount = 0;

	hFeed = EvtQuery(NULL, swzAbsolutePath, NULL, EvtQueryFilePath | EvtQueryForwardDirection);
	if (hFeed == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to open log file '%s', code %u\n"), swzAbsolutePath, res);
		goto cleanup;
	}

	while (EvtNext(hFeed, 1, &hEvent, INFINITE, 0, &dwEvtCount))
	{
      render_event_callback(hEvent);
	}
	if (GetLastError() != ERROR_NO_MORE_ITEMS)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error while reading events from '%s', code %u\n"), swzAbsolutePath, res);
		goto cleanup;
	}

cleanup:
	if (hFeed != NULL)
		EvtClose(hFeed);
	return res;
}