#include <Windows.h>

#define FIRST_TRY_RENDERING_BUF_LEN 0x100

PVOID safe_alloc(SIZE_T dwBytes);
PVOID safe_realloc(PVOID pBuf, SIZE_T dwBytes);
VOID safe_free(PVOID pBuf);
PVOID safe_dup(const VOID *pBuf, SIZE_T dwBytes);
PSTR safe_strdup(PCSTR pBuf);
// Allocates a UTF8 version of the given UTF16 string, which then
// needs to be deallocated by the caller using safe_free(). The len
// parameter can be -1 if the wide string is L"\0" terminated, otherwise
// it must be the number of wide chars to convert. The resulting UTF8
// string is always NULL-terminated.
PSTR safe_strconv(PCWSTR swzWide, int len);
PSTR sprintf_alloc(PCSTR szFormat, ...);
