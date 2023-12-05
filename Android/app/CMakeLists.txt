cmake_minimum_required(VERSION 3.4.1)

add_library(
        rs-audio-lib
        SHARED
        src/main/cpp/jni-bridge.cpp
        src/main/cpp/AudioEngine.cpp
)

find_library(
        log-lib
        log
)

target_link_libraries(
        rs-audio-lib
        ${log-lib}
        aaudio
)