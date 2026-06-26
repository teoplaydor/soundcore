# SoundCore APO DLL.
#
# Loaded by audiodg.exe into the audio graph. Implements the standard APO
# COM interfaces and forwards the audio block through the JUCE VST3 chain
# defined in `native/vst-host`.

set(TARGET_NAME SoundCoreApo)

set(APO_SOURCES
    src/dllmain.cpp
    src/SoundCoreApo.cpp
    src/Registration.cpp
    src/ChainConfig.cpp
    src/GuidsImpl.cpp
)

set(APO_HEADERS
    include/SoundCoreApo.h
    include/ChainConfig.h
)

add_library(${TARGET_NAME} SHARED
    ${APO_SOURCES}
    ${APO_HEADERS}
    SoundCoreApo.def
)

target_include_directories(${TARGET_NAME}
    PRIVATE
        include
        ../vst-host/include
)

# Windows SDK libraries — APO base + COM. `audiobaseprocessingobject.lib`
# carries `CBaseAudioProcessingObject` even though we don't use it; the
# header still pulls in some inline helpers.
target_link_libraries(${TARGET_NAME}
    PRIVATE
        audiobaseprocessingobject
        audioeng
        audiomediatypecrt
        propsys
        wmcodecdspuuid
        ole32
        oleaut32
        uuid
        msdmo
)

# Link the JUCE VST3 host static lib + all of JUCE's transitive Win32 deps.
target_link_libraries(${TARGET_NAME}
    PRIVATE
        soundcore_vst_host
)

# JUCE pulls in this long tail on Windows.
target_link_libraries(${TARGET_NAME}
    PRIVATE
        gdi32 user32 advapi32 shell32 winmm version imm32
        shlwapi ws2_32 wininet rpcrt4 dxgi dwmapi msimg32
        gdiplus winspool mfuuid mfplat mf mfreadwrite ksuser
        Pathcch
)

target_link_options(${TARGET_NAME} PRIVATE
    "/DEF:${CMAKE_CURRENT_SOURCE_DIR}/SoundCoreApo.def"
)

set_target_properties(${TARGET_NAME} PROPERTIES
    OUTPUT_NAME "SoundCoreApo"
    PREFIX ""
    SUFFIX ".dll"
)

target_compile_definitions(${TARGET_NAME} PRIVATE
    NDEBUG=1
)
