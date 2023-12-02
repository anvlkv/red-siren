//
//  unitExtensionParameterAddresses.h
//  unitExtension
//
//  Created by a.nvlkv on 02/12/2023.
//

#pragma once

#include <AudioToolbox/AUParameters.h>

#ifdef __cplusplus
namespace unitExtensionParameterAddress {
#endif

typedef NS_ENUM(AUParameterAddress, unitExtensionParameterAddress) {
    gain = 0
};

#ifdef __cplusplus
}
#endif
