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

PSTR safe_strdup(PCSTR pBuf)
{
    SIZE_T dwLen = strlen(pBuf);
    PSTR pRes = (PSTR)safe_alloc(dwLen + 1);
    memcpy(pRes, pBuf, dwLen);
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

PSTR safe_strconv(PCWSTR swzWide, int len)
{
   int buf_len = -1;
   PSTR szResult = NULL;
   
   buf_len = WideCharToMultiByte(CP_UTF8, 0, swzWide, len, NULL, 0, NULL, NULL);
   if (buf_len <= 0)
      return safe_strdup("");
   szResult = (LPSTR)safe_alloc(buf_len + 1);
   if (WideCharToMultiByte(CP_UTF8, 0, swzWide, len, szResult, buf_len + 1, NULL, NULL) <= 0)
      return safe_strdup("<UTF16 conversion failed>");
   return szResult;
}

PSTR sprintf_alloc(PCSTR szFormat, ...)
{
   va_list va_args;
   int bytes_req = 0;
   PSTR pBuffer = NULL;

   va_start(va_args, szFormat);
   pBuffer = (PSTR)safe_alloc(FIRST_TRY_RENDERING_BUF_LEN + 1);
   bytes_req = vsnprintf(pBuffer, FIRST_TRY_RENDERING_BUF_LEN, szFormat, va_args);
   va_end(va_args);
   if (bytes_req >= 0 && bytes_req + 1 <= FIRST_TRY_RENDERING_BUF_LEN)
   {
      return pBuffer;
   }
   else if (bytes_req < 0)
   {
      _ftprintf(stderr, TEXT("Error: failed to format string: sprintf(\"%hs\")\n"), szFormat);
      safe_free(pBuffer);
      return NULL;
   }
   pBuffer = (PSTR)safe_realloc(pBuffer, bytes_req + 1);
   va_start(va_args, szFormat);
   bytes_req = vsnprintf(pBuffer, bytes_req + 1, szFormat, va_args);
   va_end(va_args);
   if (bytes_req > 0)
   {
      return pBuffer;
   }
   else
   {
      _ftprintf(stderr, TEXT("Error: failed to format string (2): sprintf(\"%hs\")\n"), szFormat);
      safe_free(pBuffer);
      return NULL;
   }
}