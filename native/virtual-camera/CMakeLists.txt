# Virtual camera DLL.
#
# Registers a DirectShow Source Filter and, on Windows 11 22000+, an
# MFVirtualCamera. Both are user-mode COM objects living in the same DLL
# so the installer has only one file to drop and one CLSID to register.

set(TARGET_NAME SoundCoreVirtualCamera)

set(VC_SOURCES
    src/dllmain.cpp
    src/Filter.cpp
    src/OutputPin.cpp
    src/MediaTypes.cpp
    src/Registration.cpp
    src/SharedFrameReader.cpp
    src/MFVirtualCamera.cpp
    src/GuidsImpl.cpp
)

set(VC_HEADERS
    include/Filter.h
    include/OutputPin.h
    include/MediaTypes.h
    include/SharedFrameReader.h
    include/Guids.h
)

add_library(${TARGET_NAME} SHARED
    ${VC_SOURCES}
    ${VC_HEADERS}
    SoundCoreVirtualCamera.def
)

target_include_directories(${TARGET_NAME} PRIVATE include)

target_link_libraries(${TARGET_NAME}
    PRIVATE
        strmiids        # DirectShow class IDs
        mf
        mfplat
        mfuuid
        mfreadwrite
        mfsensorgroup   # MFCreateVirtualCamera
        ole32
        oleaut32
        uuid
        propsys
        Shlwapi
        Advapi32
)

## INITGUID is defined in exactly one TU (dllmain.cpp) — see src/dllmain.cpp.

target_link_options(${TARGET_NAME} PRIVATE
    "/DEF:${CMAKE_CURRENT_SOURCE_DIR}/SoundCoreVirtualCamera.def"
)

set_target_properties(${TARGET_NAME} PROPERTIES
    OUTPUT_NAME "SoundCoreVirtualCamera"
    PREFIX ""
    SUFFIX ".dll"
)
