#import <AudioToolbox/AudioToolbox.h>
#import <AVFoundation/AVFoundation.h>

@interface UnitExtensionAudioUnit : AUAudioUnit
@property (nonatomic, copy) NSDictionary<NSString *, id> *callAudioUnit;
@property (nonatomic, copy) CallHostBlock callHostBlock;

@end
