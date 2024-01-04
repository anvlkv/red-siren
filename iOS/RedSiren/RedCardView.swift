import SwiftUI
import AVFoundation
import Foundation
import CoreTypes
import OSLog

struct RedCardView<C: View>: View {
    let position: MenuPosition
    let rect: Rect
    let height: CGFloat
    let width: CGFloat
    let positionX: CGFloat
    let positionY: CGFloat
    let flip: Double
    let content: C

    init(position: MenuPosition, flip: Double, @ViewBuilder content: () -> C) {
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
        self.content = content()
        self.flip = flip
    }

    var isAuthorized = {
        let status = AVCaptureDevice.authorizationStatus(for: .audio)
        return status == .authorized
    }


    var body: some View {
        ZStack (alignment: .center) {
            Rectangle()
                .frame(width: self.width, height: self.height)
                .foregroundColor(Color("Primary"))
                .cornerRadius(42)
                .shadow(radius: 12)

            if (self.flip < 90 || self.flip > 270) {
                Group {
                    self.content
                }
                    .frame(maxWidth: self.width, maxHeight: self.height)
            }
        }
            .rotation3DEffect(.degrees(self.flip), axis: (0, 1, 0))
            .frame(width: self.width, height: self.height)
            .position(x: self.positionX, y: self.positionY)
            

    }
}


