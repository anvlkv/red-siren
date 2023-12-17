import SwiftUI
import AVFoundation
import Foundation
import SharedTypes

struct MenuView: View {
    @Environment(\.coreEnv) var core: CoreEnv?
    
    var position: MenuPosition
    var rect: Rect
    var height: CGFloat
    var width: CGFloat
    var expanded: Bool
    var positionX: CGFloat
    var positionY: CGFloat
    
    let padding = CGFloat(12)
    let gap = CGFloat(24)

    init(position: MenuPosition, expanded: Bool) {
        self.position = position
        switch position {
        case .topLeft(let r):
            self.rect = r
        case .topRight(let r):
            self.rect = r
        case .bottomLeft(let r):
            self.rect = r
        case .center(let r):
            self.rect = r
        }

        self.height = CGFloat(self.rect.rect[1][1] - self.rect.rect[0][1])
        self.width = CGFloat(self.rect.rect[1][0] - self.rect.rect[0][0])
        self.positionX = CGFloat(self.rect.rect[0][0] + self.width / 2)
        self.positionY = CGFloat(self.rect.rect[0][1] + self.height / 2)
        self.expanded = expanded
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
        ZStack {
            Rectangle()
                .frame(width: self.width, height: self.height)
                .foregroundColor(Color("Main"))
                .cornerRadius(42)
                .shadow(radius: 12)
            
            VStack(spacing: self.gap) {
                Text("Red Siren")
                    .font(Font.custom("Rosarivo-Regular", size: 42))
                    .foregroundColor(Color("Primary"))
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                MenuButton(action: onPlay, label: "Play")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                if !isAuthorized() {
                    Text("Red Siren is a noise chime. Please allow audio recording after you click Play or Tune")
                        .font(Font.custom("Rosarivo-Regular", size: 22))
                        .foregroundColor(Color("Primary"))
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
                .frame(maxWidth: self.width, maxHeight: self.height)
        }.position(x: self.positionX, y: self.positionY)
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
                .foregroundColor(Color("Main"))
                .font(Font.custom("Rosarivo-Regular", size: 38))
                .frame(minWidth: 100, maxWidth: .infinity, minHeight: 44, maxHeight: .infinity)
        }
            .buttonBorderShape(.roundedRectangle(radius: 30))
            .tint(Color("Primary"))
            .buttonStyle(.borderedProminent)
            .frame(minWidth: 100, maxWidth: .infinity, minHeight: 44, maxHeight: .infinity)
    }
}



