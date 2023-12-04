import CoreAudioKit
import os

private let log = Logger(subsystem: "com.anvlkv.redsiren.RedSiren.AUExtension", category: "AudioUnitFactory")

public class AudioUnitFactory: NSObject, AUAudioUnitFactory {
    var auAudioUnit: AUAudioUnit?

    public func beginRequest(with context: NSExtensionContext) {

    }

    @objc
    public func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        auAudioUnit = try UnitExtensionAudioUnit(componentDescription: componentDescription, options: [])

        guard let audioUnit = auAudioUnit as? UnitExtensionAudioUnit else {
            fatalError("Failed to create UnitExtension")
        }

        return audioUnit
    }
    
}
