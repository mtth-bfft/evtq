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
