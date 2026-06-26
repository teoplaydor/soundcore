// Single translation unit that materialises the storage for our own
// CLSIDs (those declared with DEFINE_GUID in Guids.h). System DShow
// GUIDs come from strmiids.lib, so we do *not* define INITGUID for the
// system headers anywhere in this DLL.

#define INITGUID
#include <guiddef.h>
#include "Guids.h"
