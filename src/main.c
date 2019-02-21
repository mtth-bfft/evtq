#include <stdio.h>
#include "sources/live.h"

int main(int argc, const char* argv[])
{
	(void)(argc);
	(void)(argv);
	int res = open_source_live(NULL, NULL, NULL, NULL, TRUE);
	if (res != 0)
		goto cleanup;
	
cleanup:
	return res;
}