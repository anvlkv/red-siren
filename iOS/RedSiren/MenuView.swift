import SwiftUI
import AVFoundation
import Foundation
import SharedTypes

struct MenuView: View {
    @Environment(\.coreEnv) var core: CoreEnv?
    
    var position: MenuPosition
    var expanded: Bool
    
    let padding = CGFloat(12)
    let gap = CGFloat(24)
    
    let flip: Double

    init(position: MenuPosition, expanded: Bool, flip: Double = 0) {
        self.position = position
        self.expanded = expanded
        self.flip = flip
    }

    var isAuthorized = {
        let status = AVCaptureDevice.authorizationStatus(for: .audio)
        return status == .authorized
    }

    func onPlay() {
        self.core?.update(.menu(.play))
    }

    func onTune() {
        self.core?.update(.menu(.tune))
    }
    
    func onListen() {
        self.core?.update(.menu(.listen))
    }
    
    func onAbout() {
        self.core?.update(.menu(.about))
    }

    var body: some View {
        RedCardView(position: self.position, flip: self.flip) {
                VStack(spacing: self.gap) {
                    Text("Red Siren")
                        .font(Font.custom("Rosarivo-Regular", size: 42))
                        .foregroundColor(Color("Main"))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    MenuButton(action: onPlay, label: "Play")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    if !isAuthorized() {
                        Text("Red Siren is a noise chime. Please allow audio recording after you click Play or Tune")
                            .font(Font.custom("Rosarivo-Regular", size: 22))
                            .foregroundColor(Color("Main"))
                    }

                    MenuButton(action: onTune, label: "Tune")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    MenuButton(action: onListen, label: "Listen")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    MenuButton(action: onAbout, label: "About")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    
                }
                    .padding(EdgeInsets(
                        top: self.padding, leading: self.padding,
                        bottom: self.padding, trailing: self.padding
                    ))
            }
        
    }
}


struct MenuButton: View {
    var action: () -> Void
    var label: String
    
    init(action: @escaping () -> Void, label: String) {
        self.action = action
        self.label = label
    }
    
    var body: some View {
        Button(action: self.action) {
            Text(self.label)
                .foregroundColor(Color("Primary"))
                .font(Font.custom("Rosarivo-Regular", size: 38))
                .frame(minWidth: 100, maxWidth: .infinity, minHeight: 44, maxHeight: .infinity)
        }
            .buttonBorderShape(.roundedRectangle(radius: 30))
            .tint(Color("Main"))
            .buttonStyle(.borderedProminent)
            .frame(minWidth: 100, maxWidth: .infinity, minHeight: 44, maxHeight: .infinity)
    }
}



