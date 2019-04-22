#include <Windows.h>
#include <winevt.h>
#include <tchar.h>
#include <stdio.h>
#include <sddl.h>
#include "outputs\json.h"
#include "jansson.h"
#include "mem.h"
#include "render.h"
#include "metadata.h"

static json_t* render_field_as_json(PEVT_VARIANT pField);

int render_event_json(FILE *out, EVT_HANDLE hEvent)
{
    int res = 0;
    EVT_HANDLE hContextSystem = NULL;
    EVT_HANDLE hContextUser = NULL;
    PEVT_VARIANT pSysProps = NULL;
    PEVT_VARIANT pUserProps = NULL;
    DWORD dwBufferSize = 0;
    DWORD dwPropertyCount = 0;
    json_t *pObj = NULL;

    hContextSystem = EvtCreateRenderContext(0, NULL, EvtRenderContextSystem);
    if (hContextSystem == NULL)
    {
        res = GetLastError();
        _ftprintf(stderr, TEXT("Error: unable to create system rendering context, code %u\n"), res);
        goto cleanup;
    }
    hContextUser = EvtCreateRenderContext(0, NULL, EvtRenderContextUser);
    if (hContextUser == NULL)
    {
        res = GetLastError();
        _ftprintf(stderr, TEXT("Error: unable to create user rendering context, code %u\n"), res);
        goto cleanup;
    }
    if (EvtRender(hContextSystem, hEvent, EvtRenderEventValues, 0, NULL, &dwBufferSize, &dwPropertyCount) ||
        GetLastError() != ERROR_INSUFFICIENT_BUFFER)
    {
        res = GetLastError();
        _ftprintf(stderr, TEXT("Error: unable to render event system values, code %u\n"), res);
        goto cleanup;
    }
    pSysProps = (PEVT_VARIANT)safe_alloc(dwBufferSize);
    if (!EvtRender(hContextSystem, hEvent, EvtRenderEventValues, dwBufferSize, pSysProps, &dwBufferSize, &dwPropertyCount))
    {
        res = GetLastError();
        _ftprintf(stderr, TEXT("Error: unable to render event system values, code %u\n"), res);
        goto cleanup;
    }

    if (EvtRender(hContextUser, hEvent, EvtRenderEventValues, 0, NULL, &dwBufferSize, &dwPropertyCount))
    {
        dwPropertyCount = 0;
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
        if (!EvtRender(hContextUser, hEvent, EvtRenderEventValues, dwBufferSize, pUserProps, &dwBufferSize, &dwPropertyCount))
        {
            res = GetLastError();
            _ftprintf(stderr, TEXT("Error: unable to render event user values, code %u\n"), res);
            goto cleanup;
        }
    }

    pObj = json_object();
    json_object_set_new(pObj, "hostname", json_string(render_field(&(pSysProps[EvtSystemComputer]))));
    json_object_set_new(pObj, "record_number", json_integer(pSysProps[EvtSystemEventRecordId].UInt64Val));
    json_object_set_new(pObj, "timestamp", json_string(render_field(&(pSysProps[EvtSystemTimeCreated]))));
    json_object_set_new(pObj, "provider", json_string(render_field(&(pSysProps[EvtSystemProviderName]))));
    json_object_set_new(pObj, "eventid", json_integer(pSysProps[EvtSystemEventID].UInt16Val));
    json_object_set_new(pObj, "version", json_integer(pSysProps[EvtSystemVersion].ByteVal));
    
    for (DWORD dwProp = 0; dwProp < dwPropertyCount; dwProp++)
    {
       CHAR szDefaultFieldName[30] = { 0 };
       PCSTR szFieldName = szDefaultFieldName;
       sprintf_s(szDefaultFieldName, 30, "field%u", dwProp);
       get_event_field_name(pSysProps[EvtSystemProviderName].StringVal, pSysProps[EvtSystemEventID].UInt16Val, pSysProps[EvtSystemVersion].ByteVal, dwProp, &szFieldName);
       json_object_set_new(pObj, szFieldName, render_field_as_json(&(pUserProps[dwProp])));
    }

    res = begin_render_output();
    if (res != 0)
        goto cleanup;

    // Render and write the output
    json_dumpf(pObj, out, JSON_COMPACT);
    fwrite("\n", 1, 1, out);
    
    res = end_render_output();

cleanup:
    if (hContextSystem != NULL)
        EvtClose(hContextSystem);
    if (hContextUser != NULL)
        EvtClose(hContextUser);
    return 0;
}

static json_t* render_field_as_json(PEVT_VARIANT pField)
{
   SYSTEMTIME sysTime = { 0 };
   PSTR szBuffer = NULL;
   json_t *res = NULL;

   if (pField->Type & EVT_VARIANT_TYPE_ARRAY)
   {
      json_t *pArray = json_array();
      for (DWORD dwArrayItem = 0; dwArrayItem < pField->Count; dwArrayItem++)
      {
         switch (pField->Type & ~EVT_VARIANT_TYPE_ARRAY)
         {
         case EvtVarTypeString:
            json_array_append_new(pArray, json_sprintf("%ws", pField->StringArr[dwArrayItem]));
            break;
         case EvtVarTypeAnsiString:
            json_array_append_new(pArray, json_string(pField->AnsiStringArr[dwArrayItem]));
            break;
         case EvtVarTypeSByte:
            json_array_append_new(pArray, json_integer(pField->SByteArr[dwArrayItem]));
            break;
         case EvtVarTypeByte:
            json_array_append_new(pArray, json_integer(pField->ByteArr[dwArrayItem]));
            break;
         case EvtVarTypeInt16:
            json_array_append_new(pArray, json_integer(pField->Int16Arr[dwArrayItem]));
            break;
         case EvtVarTypeUInt16:
            json_array_append_new(pArray, json_integer(pField->UInt16Arr[dwArrayItem]));
            break;
         case EvtVarTypeInt32:
            json_array_append_new(pArray, json_integer(pField->Int32Arr[dwArrayItem]));
            break;
         case EvtVarTypeUInt32:
            json_array_append_new(pArray, json_integer(pField->UInt32Arr[dwArrayItem]));
            break;
         case EvtVarTypeInt64:
            json_array_append_new(pArray, json_integer(pField->Int64Arr[dwArrayItem]));
            break;
         case EvtVarTypeUInt64:
            json_array_append_new(pArray, json_integer(pField->UInt64Arr[dwArrayItem]));
            break;
         case EvtVarTypeSingle:
            json_array_append_new(pArray, json_real(pField->SingleArr[dwArrayItem]));
            break;
         case EvtVarTypeDouble:
            json_array_append_new(pArray, json_real(pField->DoubleArr[dwArrayItem]));
            break;
         case EvtVarTypeBoolean:
            json_array_append_new(pArray, json_boolean(pField->BooleanArr[dwArrayItem]));
            break;
         case EvtVarTypeGuid:
            json_array_append_new(pArray, json_sprintf(
               "{%08lX-%04hX-%04hX-%02hhX%02hhX-%02hhX%02hhX%02hhX%02hhX%02hhX%02hhX}",
               pField->GuidArr[dwArrayItem].Data1, pField->GuidArr[dwArrayItem].Data2,
               pField->GuidArr[dwArrayItem].Data3, pField->GuidArr[dwArrayItem].Data4[0],
               pField->GuidArr[dwArrayItem].Data4[1], pField->GuidArr[dwArrayItem].Data4[2],
               pField->GuidArr[dwArrayItem].Data4[3], pField->GuidArr[dwArrayItem].Data4[4],
               pField->GuidArr[dwArrayItem].Data4[5], pField->GuidArr[dwArrayItem].Data4[6],
               pField->GuidArr[dwArrayItem].Data4[7]));
            break;
         case EvtVarTypeSizeT:
            json_array_append_new(pArray, json_integer(pField->SizeTArr[dwArrayItem]));
            break;
         case EvtVarTypeFileTime:
            if (!FileTimeToSystemTime((FILETIME*)&(pField->FileTimeArr[dwArrayItem]), &sysTime))
            {
               _ftprintf(stderr, TEXT("Error: failed to convert FileTime to SystemTime\n"));
               json_array_append_new(pArray, json_string("<unknown date?>"));
            }
            json_array_append_new(pArray, json_sprintf(
               "%04d-%02d-%02d %02d:%02d:%02d.%03d",
               sysTime.wYear, sysTime.wMonth, sysTime.wDay, sysTime.wHour,
               sysTime.wMinute, sysTime.wSecond, sysTime.wMilliseconds));
            break;
         case EvtVarTypeSysTime:
            json_array_append_new(pArray, json_sprintf(
               "%04d-%02d-%02d %02d:%02d:%02d.%03d",
               pField->SysTimeArr[dwArrayItem].wYear, pField->SysTimeArr[dwArrayItem].wMonth,
               pField->SysTimeArr[dwArrayItem].wDay, pField->SysTimeArr[dwArrayItem].wHour,
               pField->SysTimeArr[dwArrayItem].wMinute, pField->SysTimeArr[dwArrayItem].wSecond,
               pField->SysTimeArr[dwArrayItem].wMilliseconds));
            break;
         case EvtVarTypeSid:
            if (!ConvertSidToStringSidA(pField->SidArr[dwArrayItem], &szBuffer))
            {
               _ftprintf(stderr, TEXT("Error: failed to convert SID to string\n"));
               json_array_append_new(pArray, json_string("<unknown SID?>"));
            }
            json_array_append_new(pArray, json_string(szBuffer));
            LocalFree(szBuffer);
            break;
         case EvtVarTypeHexInt32:
            json_array_append_new(pArray, json_integer(pField->UInt32Arr[dwArrayItem]));
            break;
         case EvtVarTypeHexInt64:
            json_array_append_new(pArray, json_integer(pField->UInt64Arr[dwArrayItem]));
            break;
         case EvtVarTypeEvtXml:
            json_array_append_new(pArray, json_sprintf("%ws", pField->XmlValArr[dwArrayItem]));
            break;
         default:
            json_array_append_new(pArray, json_sprintf("<unknown field type %u>", pField->Type));
         }
      }
      return pArray;
   }
   else
   {
      switch (pField->Type & ~EVT_VARIANT_TYPE_ARRAY)
      {
      case EvtVarTypeString:
         return json_sprintf("%ws", pField->StringVal);
      case EvtVarTypeAnsiString:
         return json_string(pField->AnsiStringVal);
      case EvtVarTypeSByte:
         return json_integer(pField->SByteVal);
      case EvtVarTypeByte:
         return json_integer(pField->ByteVal);
      case EvtVarTypeInt16:
         return json_integer(pField->Int16Val);
      case EvtVarTypeUInt16:
         return json_integer(pField->UInt16Val);
      case EvtVarTypeInt32:
         return json_integer(pField->Int32Val);
      case EvtVarTypeUInt32:
         return json_integer(pField->UInt32Val);
      case EvtVarTypeInt64:
         return json_integer(pField->Int64Val);
      case EvtVarTypeUInt64:
         return json_integer(pField->UInt64Val);
      case EvtVarTypeSingle:
         return json_real(pField->SingleVal);
      case EvtVarTypeDouble:
         return json_real(pField->DoubleVal);
      case EvtVarTypeBoolean:
         return json_boolean(pField->BooleanVal);
      case EvtVarTypeGuid:
         return json_sprintf(
            "{%08lX-%04hX-%04hX-%02hhX%02hhX-%02hhX%02hhX%02hhX%02hhX%02hhX%02hhX}",
            pField->GuidVal->Data1, pField->GuidVal->Data2,
            pField->GuidVal->Data3, pField->GuidVal->Data4[0],
            pField->GuidVal->Data4[1], pField->GuidVal->Data4[2],
            pField->GuidVal->Data4[3], pField->GuidVal->Data4[4],
            pField->GuidVal->Data4[5], pField->GuidVal->Data4[6],
            pField->GuidVal->Data4[7]);
         break;
      case EvtVarTypeSizeT:
         return json_integer(pField->SizeTVal);
      case EvtVarTypeFileTime:
         if (!FileTimeToSystemTime((FILETIME*)&(pField->FileTimeVal), &sysTime))
         {
            _ftprintf(stderr, TEXT("Error: failed to convert FileTime to SystemTime\n"));
            return json_string("<unknown date?>");
         }
         return json_sprintf(
            "%04d-%02d-%02d %02d:%02d:%02d.%03d",
            sysTime.wYear, sysTime.wMonth, sysTime.wDay, sysTime.wHour,
            sysTime.wMinute, sysTime.wSecond, sysTime.wMilliseconds);
         break;
      case EvtVarTypeSysTime:
         return json_sprintf(
            "%04d-%02d-%02d %02d:%02d:%02d.%03d",
            pField->SysTimeVal->wYear, pField->SysTimeVal->wMonth,
            pField->SysTimeVal->wDay, pField->SysTimeVal->wHour,
            pField->SysTimeVal->wMinute, pField->SysTimeVal->wSecond,
            pField->SysTimeVal->wMilliseconds);
         break;
      case EvtVarTypeSid:
         if (!ConvertSidToStringSidA(pField->SidVal, &szBuffer))
         {
            _ftprintf(stderr, TEXT("Error: failed to convert SID to string\n"));
            return json_string("<unknown SID?>");
         }
         res = json_string(szBuffer);
         LocalFree(szBuffer);
         return res;
      case EvtVarTypeHexInt32:
         return json_integer(pField->UInt32Val);
      case EvtVarTypeHexInt64:
         return json_integer(pField->UInt64Val);
      case EvtVarTypeEvtXml:
         return json_sprintf("%ws", pField->XmlVal);
      default:
         return json_sprintf("<unknown field type %u>", pField->Type);
      }
   }
}