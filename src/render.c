#include <Windows.h>
#include <tchar.h>
#include <winevt.h>
#include <stdio.h>
#include <inttypes.h>
#include <sddl.h>
#include "render.h"
#include "mem.h"

static HANDLE ghOutputMutex = INVALID_HANDLE_VALUE;

int init_render_output()
{
   int res = 0;

   ghOutputMutex = CreateMutex(NULL, FALSE, NULL);
   if (ghOutputMutex == NULL)
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Error: unable to create mutex, code %u\n"), res);
   }
   return res;
}

int begin_render_output()
{
   int res = 0;
   DWORD dwWait = 0;

   dwWait = WaitForSingleObject(ghOutputMutex, INFINITE);
   if (dwWait != WAIT_OBJECT_0)
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT(" [!] Error while waiting to acquire output mutex, code %u\n"), res);
   }
   return res;
}

int end_render_output()
{
   int res = 0;
   if (!ReleaseMutex(ghOutputMutex))
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT(" [!] Error while releasing output mutex, code %u\n"), res);
   }
   return res;
}

void strip_non_printable_chars(PSTR szValue)
{
   SIZE_T dwLen = strlen(szValue);
   for (SIZE_T dwChar = 0; dwChar < dwLen; dwChar++)
   {
      if (szValue[dwChar] < ' ' || szValue[dwChar] > '~')
      {
         szValue[dwChar] = ' ';
      }
   }
}

PSTR render_field(PEVT_VARIANT pVar)
{
   int buf_len = 0;
   PSTR szResult = NULL;
   PSTR szBuffer = NULL;
   SYSTEMTIME sysTime = { 0 };

   switch (pVar->Type)
   {
   case EvtVarTypeString:
      buf_len = WideCharToMultiByte(CP_UTF8, 0, pVar->StringVal, -1, NULL, 0, NULL, NULL);
      if (buf_len <= 0)
         return safe_strdup("");
      szResult = (LPSTR)safe_alloc(buf_len + 1);
      if (WideCharToMultiByte(CP_UTF8, 0, pVar->StringVal, -1, szResult, buf_len + 1, NULL, NULL) <= 0)
      {
         _ftprintf(stderr, TEXT("Error: failed to convert UTF-16 string\n"));
         return safe_strdup("???");
      }
      return szResult;
   case EvtVarTypeAnsiString:
      return sprintf_alloc("%s", pVar->AnsiStringVal);
   case EvtVarTypeSByte:
      return sprintf_alloc("%" PRIi8, pVar->SByteVal);
   case EvtVarTypeInt16:
      return sprintf_alloc("%" PRIi16, pVar->Int16Val);
   case EvtVarTypeInt32:
      return sprintf_alloc("%" PRIi32, pVar->Int32Val);
   case EvtVarTypeByte:
      return sprintf_alloc("%" PRIu8, pVar->ByteVal);
   case EvtVarTypeUInt16:
      return sprintf_alloc("%" PRIu16, pVar->UInt16Val);
   case EvtVarTypeUInt32:
      return sprintf_alloc("%" PRIu32, pVar->UInt32Val);
   case EvtVarTypeUInt64:
      return sprintf_alloc("%" PRIu64, pVar->UInt64Val);
   case EvtVarTypeSingle:
      return sprintf_alloc("%f", pVar->SingleVal);
   case EvtVarTypeDouble:
      return sprintf_alloc("%f", pVar->DoubleVal);
   case EvtVarTypeBoolean:
      return safe_strdup((pVar->BooleanVal ? "true" : "false"));
   case EvtVarTypeSizeT:
      return sprintf_alloc("%p", pVar->SizeTVal);
   case EvtVarTypeHexInt32:
      return sprintf_alloc("%04X", pVar->UInt32Val);
   case EvtVarTypeHexInt64:
      return sprintf_alloc("%08llX", pVar->UInt64Val);
   case EvtVarTypeGuid:
      return sprintf_alloc("{%08lX-%04hX-%04hX-%02hhX%02hhX-%02hhX%02hhX%02hhX%02hhX%02hhX%02hhX}",
         pVar->GuidVal->Data1, pVar->GuidVal->Data2, pVar->GuidVal->Data3,
         pVar->GuidVal->Data4[0], pVar->GuidVal->Data4[1], pVar->GuidVal->Data4[2], pVar->GuidVal->Data4[3],
         pVar->GuidVal->Data4[4], pVar->GuidVal->Data4[5], pVar->GuidVal->Data4[6], pVar->GuidVal->Data4[7]);
   case EvtVarTypeSid:
      if (!ConvertSidToStringSidA(pVar->SidVal, &szBuffer))
      {
         _ftprintf(stderr, TEXT("Error: failed to convert SID to string\n"));
         return safe_strdup("<unknown SID?>");
      }
      szResult = safe_strdup(szBuffer);
      LocalFree(szBuffer);
      return szResult;
   case EvtVarTypeFileTime:
      if (!FileTimeToSystemTime((FILETIME*)&(pVar->FileTimeVal), &sysTime))
      {
         _ftprintf(stderr, TEXT("Error: failed to convert FileTime to SystemTime\n"));
         return safe_strdup("<unknown date?>");
      }
      return sprintf_alloc("%04d-%02d-%02d %02d:%02d:%02d.%03d",
         sysTime.wYear, sysTime.wMonth, sysTime.wDay,
         sysTime.wHour, sysTime.wMinute, sysTime.wSecond,
         sysTime.wMilliseconds);
   case EvtVarTypeSysTime:
      return sprintf_alloc("%04d-%02d-%02d %02d:%02d:%02d.%03d",
         pVar->SysTimeVal->wYear, pVar->SysTimeVal->wMonth, pVar->SysTimeVal->wDay,
         pVar->SysTimeVal->wHour, pVar->SysTimeVal->wMinute, pVar->SysTimeVal->wSecond,
         pVar->SysTimeVal->wMilliseconds);
   case EvtVarTypeBinary:
      return sprintf_alloc("%02X", *pVar->BinaryVal);
   default:
      return sprintf_alloc("<type=%u ?>", pVar->Type);
   }
}
