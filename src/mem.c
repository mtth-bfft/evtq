#include <tchar.h>
#include <stdio.h>
#include "mem.h"

PVOID safe_alloc(SIZE_T dwBytes)
{
	PVOID pRes = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, dwBytes);
	if (pRes == NULL)
	{
		_ftprintf(stderr, TEXT("Error: unable to allocate %zu bytes: code %u\n"),
			dwBytes, GetLastError());
		_exit(ERROR_OUTOFMEMORY);
	}
	return pRes;
}

PVOID safe_realloc(PVOID pBuf, SIZE_T dwBytes)
{
	PVOID pRes = NULL;
	if (pBuf == NULL)
		return safe_alloc(dwBytes);
	pRes = HeapReAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, pBuf, dwBytes);
	if (pRes == NULL)
	{
		_ftprintf(stderr, TEXT("Error: unable to extend allocation to %zu bytes: code %u\n"),
			dwBytes, GetLastError());
		_exit(ERROR_OUTOFMEMORY);
	}
	return pRes;
}

PVOID safe_dup(const VOID *pBuf, SIZE_T dwBytes)
{
	PVOID pRes = safe_alloc(dwBytes);
	memcpy(pRes, pBuf, dwBytes);
	return pRes;
}

VOID safe_free(PVOID pBuf)
{
	if (pBuf == NULL || !HeapFree(GetProcessHeap(), 0, pBuf))
	{
		_ftprintf(stderr, TEXT("Error: tried to free %p, heap corrupted\n"), pBuf);
		_exit(ERROR_OUTOFMEMORY);
	}
}

int map_file_readonly(PCTSTR swzFilePath, PVOID *ppBuf, PSIZE_T pdwBufLen)
{
	int res = 0;
	HANDLE hFile = INVALID_HANDLE_VALUE;
	LARGE_INTEGER liFileSize = { 0 };
	HANDLE hFileMap = NULL;
	PVOID pBuf = NULL;

	hFile = CreateFile(swzFilePath, GENERIC_READ, FILE_SHARE_READ, NULL,
		OPEN_EXISTING, FILE_FLAG_SEQUENTIAL_SCAN, NULL);
	if (hFile == INVALID_HANDLE_VALUE)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT(" [!] Could not open file '%s': code %u\n"), swzFilePath, res);
		goto cleanup;
	}
	if (!GetFileSizeEx(hFile, &liFileSize))
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT(" [!] Could not get '%s' size: code %u\n"), swzFilePath, res);
		goto cleanup;
	}
	hFileMap = CreateFileMappingNuma(hFile, NULL, PAGE_READONLY, 0, 0, NULL, NUMA_NO_PREFERRED_NODE);
	if (hFileMap == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT(" [!] Could not map file '%s': code %u\n"), swzFilePath, res);
		goto cleanup;
	}
	pBuf = MapViewOfFileExNuma(hFileMap, FILE_MAP_READ, 0, 0, 0, NULL, NUMA_NO_PREFERRED_NODE);
	if (pBuf == NULL)
	{
		res = GetLastError();
		_ftprintf(stderr, TEXT(" [!] Could not map view of '%s': code %u\n"), swzFilePath, res);
		goto cleanup;
	}
	*ppBuf = pBuf;
	*pdwBufLen = liFileSize.QuadPart;

cleanup:
	if (pBuf != NULL)
		UnmapViewOfFile(pBuf);
	if (hFileMap != NULL)
		CloseHandle(hFileMap);
	if (hFile != INVALID_HANDLE_VALUE)
		CloseHandle(hFile);
	return res;
}