cmake_minimum_required(VERSION 3.25)

project(SoundCoreNative
        VERSION 0.1.0
        LANGUAGES C CXX)

# Project-wide policies
if (POLICY CMP0091)
    cmake_policy(SET CMP0091 NEW) # MSVC runtime selection via CMAKE_MSVC_RUNTIME_LIBRARY
endif()
if (POLICY CMP0135)
    cmake_policy(SET CMP0135 NEW) # FetchContent: timestamp from extract, not archive
endif()

if (NOT WIN32)
    message(FATAL_ERROR "SoundCore native components target Windows only")
endif()

# Pin to the Windows SDK headers we want (Win10/11 audio + APO + MF + DShow).
add_compile_definitions(
    _WIN32_WINNT=0x0A00  # Windows 10+
    WINVER=0x0A00
    UNICODE _UNICODE
    NOMINMAX
    WIN32_LEAN_AND_MEAN
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Always use `/MD` (release dynamic CRT) — Rust's MSVC target links to
# the release CRT even in `cargo build` debug profile, and linking a
# /MDd C++ static lib into a Rust /MD binary leaves the debug CRT
# symbols (`_CrtDbgReport` etc.) unresolved. Matching `/MD` everywhere
# avoids that whole class of errors.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")

# Where the Rust-built static libraries land. The top-level build script
# arranges for `cargo build --release` to produce these before invoking
# CMake.
set(SOUNDCORE_RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/../target"
    CACHE PATH "Cargo target directory")

# Subdirectories.
add_subdirectory(vst-host)
add_subdirectory(apo)
add_subdirectory(virtual-camera)
