#include <Windows.h>
#include <tchar.h>
#include <stdio.h>
#include <inttypes.h>
#include <winevt.h>
#include <sddl.h>
#include "outputs/tsv.h"
#include "mem.h"
#include "render.h"

// Number of columns in every line of TSV output
#define TSV_FIELDS 10
#define SYSTEM_FIELDS 6
#define USER_FIELDS (TSV_FIELDS - SYSTEM_FIELDS)

int render_event_tsv(FILE *out, EVT_HANDLE hEvent, PEVT_VARIANT pSysProps)
{
   int res = 0;
   EVT_HANDLE hContextUser = NULL;
   PEVT_VARIANT pUserProps = NULL;
   DWORD dwUserPropsCount = 0;
   DWORD dwBufferSize = 0;
   SYSTEMTIME evtTimestamp = { 0 };
   PSTR szBuffer = NULL;
   SYSTEMTIME sysTime = { 0 };

   hContextUser = EvtCreateRenderContext(0, NULL, EvtRenderContextUser);
   if (hContextUser == NULL)
   {
      res = GetLastError();
      _ftprintf(stderr, TEXT("Error: unable to create user rendering context, code %u\n"), res);
      goto cleanup;
   }
   
   FileTimeToSystemTime((FILETIME*)&(pSysProps[EvtSystemTimeCreated].FileTimeVal), &evtTimestamp);
   
   if (EvtRender(hContextUser, hEvent, EvtRenderEventValues, 0, NULL, &dwBufferSize, &dwUserPropsCount))
   {
      dwUserPropsCount = 0;
   }
   else
   {
      if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
      {
         res = GetLastError();
         _ftprintf(stderr, TEXT("Error: unable to render event user values, code %u\n"), res);
         goto cleanup;
      }
      pUserProps = (PEVT_VARIANT)safe_alloc(dwBufferSize);
      if (!EvtRender(hContextUser, hEvent, EvtRenderEventValues, dwBufferSize, pUserProps, &dwBufferSize, &dwUserPropsCount))
      {
         res = GetLastError();
         _ftprintf(stderr, TEXT("Error: unable to render event user values, code %u\n"), res);
         goto cleanup;
      }
   }

   res = begin_render_output();
   if (res != 0)
       goto cleanup;

   fprintf(out, "%ws\t%" PRIu64 "\t%04u-%02u-%02u %02u:%02u:%02u.%03u\t%ws\t%u\t%u\t",
      pSysProps[EvtSystemComputer].StringVal,
      pSysProps[EvtSystemEventRecordId].UInt64Val,
      evtTimestamp.wYear, evtTimestamp.wMonth, evtTimestamp.wDay,
      evtTimestamp.wHour, evtTimestamp.wMinute, evtTimestamp.wSecond,
      evtTimestamp.wMilliseconds,
      pSysProps[EvtSystemProviderName].StringVal,
      pSysProps[EvtSystemEventID].UInt16Val,
      pSysProps[EvtSystemVersion].ByteVal);

   for (DWORD dwProp = 0; dwProp < USER_FIELDS; dwProp++)
   {
      if (dwProp < dwUserPropsCount)
      {
         PSTR szField = NULL;
         if (pUserProps[dwProp].Type & EVT_VARIANT_TYPE_ARRAY)
         {
            fwrite("[", 1, 1, out);
            for (DWORD dwArrayItem = 0; dwArrayItem < pUserProps[dwProp].Count; dwArrayItem++)
            {
                if (dwArrayItem != 0)
                    fwrite(",", 1, 1, out);
                switch ((pUserProps[dwProp].Type) & ~EVT_VARIANT_TYPE_ARRAY)
                {
                case EvtVarTypeString:
                    fprintf(out, "%ws", pUserProps[dwProp].StringArr[dwArrayItem]);
                    break;
                case EvtVarTypeAnsiString:
                    fprintf(out, "%s", pUserProps[dwProp].AnsiStringArr[dwArrayItem]);
                    break;
                case EvtVarTypeSByte:
                    fprintf(out, "%" PRIi8, pUserProps[dwProp].SByteArr[dwArrayItem]);
                    break;
                case EvtVarTypeByte:
                    fprintf(out, "%" PRIu8, pUserProps[dwProp].ByteArr[dwArrayItem]);
                    break;
                case EvtVarTypeInt16:
                    fprintf(out, "%" PRIi16, pUserProps[dwProp].Int16Arr[dwArrayItem]);
                    break;
                case EvtVarTypeUInt16:
                    fprintf(out, "%" PRIu16, pUserProps[dwProp].UInt16Arr[dwArrayItem]);
                    break;
                case EvtVarTypeInt32:
                    fprintf(out, "%" PRIi32, pUserProps[dwProp].Int32Arr[dwArrayItem]);
                    break;
                case EvtVarTypeUInt32:
                    fprintf(out, "%" PRIu32, pUserProps[dwProp].UInt32Arr[dwArrayItem]);
                    break;
                case EvtVarTypeInt64:
                    fprintf(out, "%" PRIi64, pUserProps[dwProp].Int64Arr[dwArrayItem]);
                    break;
                case EvtVarTypeUInt64:
                    fprintf(out, "%" PRIu64, pUserProps[dwProp].UInt64Arr[dwArrayItem]);
                    break;
                case EvtVarTypeSingle:
                    fprintf(out, "%f", pUserProps[dwProp].SingleArr[dwArrayItem]);
                    break;
                case EvtVarTypeDouble:
                    fprintf(out, "%lf", pUserProps[dwProp].DoubleArr[dwArrayItem]);
                    break;
                case EvtVarTypeBoolean:
                    fprintf(out, (pUserProps[dwProp].BooleanArr[dwArrayItem] ? "true" : "false"));
                    break;
                case EvtVarTypeGuid:
                    fprintf(out, "{%08lX-%04hX-%04hX-%02hhX%02hhX-%02hhX%02hhX%02hhX%02hhX%02hhX%02hhX}",
                        pUserProps[dwProp].GuidArr[dwArrayItem].Data1, pUserProps[dwProp].GuidArr[dwArrayItem].Data2,
                        pUserProps[dwProp].GuidArr[dwArrayItem].Data3, pUserProps[dwProp].GuidArr[dwArrayItem].Data4[0],
                        pUserProps[dwProp].GuidArr[dwArrayItem].Data4[1], pUserProps[dwProp].GuidArr[dwArrayItem].Data4[2],
                        pUserProps[dwProp].GuidArr[dwArrayItem].Data4[3], pUserProps[dwProp].GuidArr[dwArrayItem].Data4[4],
                        pUserProps[dwProp].GuidArr[dwArrayItem].Data4[5], pUserProps[dwProp].GuidArr[dwArrayItem].Data4[6],
                        pUserProps[dwProp].GuidArr[dwArrayItem].Data4[7]);
                    break;
                case EvtVarTypeSizeT:
                    fprintf(out, "%zu", pUserProps[dwProp].SizeTArr[dwArrayItem]);
                    break;
                case EvtVarTypeFileTime:
                    if (!FileTimeToSystemTime((FILETIME*)&(pUserProps[dwProp].FileTimeArr[dwArrayItem]), &sysTime))
                    {
                        _ftprintf(stderr, TEXT("Error: failed to convert FileTime to SystemTime\n"));
                        fprintf(out, "<unknown date?>");
                    }
                    fprintf(out, "%04d-%02d-%02d %02d:%02d:%02d.%03d",
                        sysTime.wYear, sysTime.wMonth, sysTime.wDay, sysTime.wHour,
                        sysTime.wMinute, sysTime.wSecond, sysTime.wMilliseconds);
                    break;
                case EvtVarTypeSysTime:
                    fprintf(out, "%04d-%02d-%02d %02d:%02d:%02d.%03d",
                        pUserProps[dwProp].SysTimeArr[dwArrayItem].wYear, pUserProps[dwProp].SysTimeArr[dwArrayItem].wMonth,
                        pUserProps[dwProp].SysTimeArr[dwArrayItem].wDay, pUserProps[dwProp].SysTimeArr[dwArrayItem].wHour,
                        pUserProps[dwProp].SysTimeArr[dwArrayItem].wMinute, pUserProps[dwProp].SysTimeArr[dwArrayItem].wSecond,
                        pUserProps[dwProp].SysTimeArr[dwArrayItem].wMilliseconds);
                    break;
                case EvtVarTypeSid:
                    if (!ConvertSidToStringSidA(pUserProps[dwProp].SidArr[dwArrayItem], &szBuffer))
                    {
                        _ftprintf(stderr, TEXT("Error: failed to convert SID to string\n"));
                        fprintf(out, "<unknown SID?>");
                    }
                    fprintf(out, "%s", szBuffer);
                    LocalFree(szBuffer);
                    break;
                case EvtVarTypeHexInt32:
                    fprintf(out, "%04X", pUserProps[dwProp].UInt32Arr[dwArrayItem]);
                    break;
                case EvtVarTypeHexInt64:
                    fprintf(out, "%08llX", pUserProps[dwProp].UInt64Arr[dwArrayItem]);
                    break;
                case EvtVarTypeEvtXml:
                    fprintf(out, "%ws", pUserProps[dwProp].XmlValArr[dwArrayItem]);
                    break;
                default:
                    fprintf(out, "<type=%u ?>", pUserProps[dwProp].Type);
                }
               if (szField != NULL)
               {
                  strip_non_printable_chars(szField);
                  fwrite(szField, strlen(szField), 1, out);
               }
            }
            fwrite("]", 1, 1, out);
         }
         else
         {
            szField = render_field(&(pUserProps[dwProp]));
            if (szField != NULL)
            {
               strip_non_printable_chars(szField);
               fwrite(szField, strlen(szField), 1, out);
            }
         }
      }
      fprintf(out, "\t");
   }
   fprintf(out, "\n");

   res = end_render_output();

cleanup:
   if (hContextUser != NULL)
      EvtClose(hContextUser);
   return 0;
}
