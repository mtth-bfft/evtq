#include <Windows.h>
#include <tchar.h>
#include <winevt.h>
#include <stdio.h>
#include "sources/backup.h"

int open_source_backup(PWSTR swzAbsolutePath)
{
	int res = 0;
	HANDLE hLog = NULL;
	HANDLE hFeed = NULL;
	HANDLE hEvent = NULL;
	DWORD dwEvtCount = 0;

	hLog = EvtOpenLog(NULL, swzAbsolutePath, EvtOpenFilePath);
	if (hLog == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to open log file %ws metadata, code %u\n"), swzAbsolutePath, res);
		goto cleanup;
	}

	hFeed = EvtQuery(NULL, swzAbsolutePath, NULL, EvtQueryFilePath | EvtQueryForwardDirection);
	if (hFeed == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error: unable to open log file %ws, code %u\n"), swzAbsolutePath, res);
		goto cleanup;
	}

	while (EvtNext(hFeed, 1, &hEvent, INFINITE, 0, &dwEvtCount))
	{
		
	}
	if (GetLastError() != ERROR_NO_MORE_ITEMS)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT("Error while reading events from %ws , code %u\n"), swzAbsolutePath, res);
		goto cleanup;
	}

cleanup:
	if (hLog != NULL)
		EvtClose(hLog);
	if (hFeed != NULL)
		EvtClose(hFeed);
	return res;
}