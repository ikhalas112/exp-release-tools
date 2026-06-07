# CMakeLists.txt for Maxion C++ Callback System Demo (Standalone)
# Minimum required version
cmake_minimum_required(VERSION 3.10)

# Project name and version
project(maxion-cpp-callback-demo
    VERSION 1.0.0
    DESCRIPTION "Maxion Anti-Cheat C++ Callback System Demo (Standalone)"
    LANGUAGES CXX
)

# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Build type
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")

# Compiler-specific options
if(MSVC)
    # Microsoft Visual C++
    add_compile_options(/W4)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        add_compile_options(/O2)
    else()
        add_compile_options(/Od)
    endif()
else()
    # GCC and Clang
    add_compile_options(-Wall -Wextra -pedantic)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        add_compile_options(-O2)
    else()
        add_compile_options(-O0 -g)
    endif()
endif()

# Source files
set(CALLBACK_DEMO_SOURCES
    callback_demo.cpp
)

# Create executable
add_executable(callback_demo ${CALLBACK_DEMO_SOURCES})

# Thread library (required for std::thread, std::atomic)
find_package(Threads REQUIRED)
target_link_libraries(callback_demo PRIVATE Threads::Threads)

# Compiler-specific definitions
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    target_compile_definitions(callback_demo PRIVATE DEBUG_MODE)
endif()

# Installation rules (optional)
install(TARGETS callback_demo
    RUNTIME DESTINATION bin
)

# Print configuration summary
message(STATUS "========================================")
message(STATUS "Maxion C++ Callback Demo Configuration")
message(STATUS "(Standalone - No Rust Dependency)")
message(STATUS "========================================")
message(STATUS "C++ Standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "Build Type: ${CMAKE_BUILD_TYPE}")
message(STATUS "Compiler: ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION}")
message(STATUS "CMAKE_SOURCE_DIR: ${CMAKE_SOURCE_DIR}")
message(STATUS "CMAKE_BINARY_DIR: ${CMAKE_BINARY_DIR}")
message(STATUS "========================================")
