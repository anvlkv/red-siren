#pragma once

#include <AudioToolbox/AUParameters.h>

#ifdef __cplusplus
namespace UnitExtensionParameterAddress {
#endif

typedef NS_ENUM(AUParameterAddress, UnitExtensionParameterAddress) {
    gain = 0
};

#ifdef __cplusplus
}
#endif
