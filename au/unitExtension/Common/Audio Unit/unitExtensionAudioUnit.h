//
//  unitExtensionAudioUnit.h
//  unitExtension
//
//  Created by a.nvlkv on 02/12/2023.
//

#import <AudioToolbox/AudioToolbox.h>
#import <AVFoundation/AVFoundation.h>

@interface unitExtensionAudioUnit : AUAudioUnit
- (void)setupParameterTree:(AUParameterTree *)parameterTree;
@end
