#include <Windows.h>

PVOID safe_alloc(SIZE_T dwBytes);
PVOID safe_realloc(PVOID pBuf, SIZE_T dwBytes);
PVOID safe_dup(const VOID *pBuf, SIZE_T dwBytes);
VOID safe_free(PVOID pBuf);
