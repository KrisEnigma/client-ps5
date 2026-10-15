# The payload SDK's toolchain, for the C/C++ dependencies cargo builds with CMake. CMake's
# compiler check links a test program, which only the title's own link can do (link-title.sh).
include("$ENV{PS5_PAYLOAD_SDK}/toolchain/prospero.cmake")
set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)
# RenderDoc's header knows no FreeBSD, and there is no RenderDoc on the console.
set(GRANITE_RENDERDOC_CAPTURE OFF CACHE BOOL "" FORCE)
set(CMAKE_C_COMPILER "${CMAKE_CURRENT_LIST_DIR}/bin/cc" CACHE PATH "" FORCE)
set(CMAKE_CXX_COMPILER "${CMAKE_CURRENT_LIST_DIR}/bin/c++" CACHE PATH "" FORCE)
