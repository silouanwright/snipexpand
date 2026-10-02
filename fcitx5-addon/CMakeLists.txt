cmake_minimum_required(VERSION 3.21)
project(snipexpand-fcitx5 LANGUAGES CXX)

include(CTest)

set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Fcitx5Core 5.0 REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(Fcitx5Module REQUIRED IMPORTED_TARGET Fcitx5Module)

add_library(snipexpand-fcitx5 MODULE snipexpand.cpp)
target_link_libraries(
  snipexpand-fcitx5
  PRIVATE
    Fcitx5::Core
    PkgConfig::Fcitx5Module
)

if(BUILD_TESTING)
  add_executable(snipexpand-fcitx5-test test_suffix.cpp)
  target_link_libraries(snipexpand-fcitx5-test PRIVATE Fcitx5::Utils)
  add_test(NAME snipexpand-fcitx5-suffix COMMAND snipexpand-fcitx5-test)
endif()
