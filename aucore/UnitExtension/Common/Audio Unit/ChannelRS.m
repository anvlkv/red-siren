//
//  ChannelRS.m
//  UnitExtension
//
//  Created by a.nvlkv on 03/12/2023.
//
#import "ChannelRS.h"
#import <Foundation/Foundation.h>
#import <AVFoundation/AVFoundation.h>

@implementation ChannelRS

- (instancetype)initWithEvHandler:  (NSDictionary<NSString *, id>* (^)(NSDictionary<NSString *, id> *message))evHandler
                             name: (NSString *) name {
    self = [super init];
    if (self) {
        _evHandler = [evHandler copy];
        _channelName = name;
    }
    return self;
}

- (NSDictionary<NSString *, id> *)callAudioUnit:(NSDictionary<NSString *, id> *)message {
    return self.evHandler(message);
}

- (void)setCallHostBlock:(CallHostBlock)block {
    self.callHostBlock = block;
}

@end
