#ifndef ChannelRS_h
#define ChannelRS_h

#import <AudioToolbox/AudioToolbox.h>
#import <AVFoundation/AVFoundation.h>

@interface ChannelRS : NSObject<AUMessageChannel>

@property (nonatomic, copy) NSDictionary<NSString *, id>* _Nonnull (^ _Nonnull evHandler)(NSDictionary<NSString *, id> * _Nonnull message);
@property (copy, nullable, nonatomic) CallHostBlock callHostBlock;
@property (nonatomic, copy) NSString * _Nonnull channelName;

- (instancetype _Nonnull )initWithEvHandler: (NSDictionary<NSString *, id>* _Nonnull (^_Nonnull)(NSDictionary<NSString *, id> * _Nonnull message))evHandler
                             name: (NSString *_Nonnull) name;

@end

#endif /* ChannelRS_h */
