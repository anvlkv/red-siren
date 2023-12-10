# This goes in the same folder as the build.gradle
project(shared)
cmake_minimum_required(VERSION 3.4.1)
set (SRC_DIR ./src/main/cpp)
add_library (dummy SHARED ${SRC_DIR}/dummy.cpp)
target_link_libraries(dummy)  # Is this needed?

# Note that this all works for apps and for Android libraries (i.e. those with gradle plugin com.android.library).
# However, I don't recommend using this approach for the latter, as a library shouldn't be adding stuff like libc++_shared.so
# itself as this will clash with any other dependency which does the same (two libraries with the same name).