#include <KWayland/Client/fakeinput.h>
#include <KWayland/Client/registry.h>
#include <QCoreApplication>
#include <QPointF>
#include <linux/input-event-codes.h>
#include <wayland-client.h>

#include <cstdio>
#include <cstdlib>
#include <memory>
#include <string_view>

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc != 2 && argc != 4) {
        return 2;
    }
    const std::string_view action(argv[1]);
    std::unique_ptr<wl_display, decltype(&wl_display_disconnect)> display(
        wl_display_connect(nullptr), wl_display_disconnect);
    if (!display) {
        std::fputs("Could not connect to the QA Wayland display.\n", stderr);
        return 1;
    }

    KWayland::Client::Registry registry;
    registry.create(display.get());
    registry.setup();
    if (wl_display_roundtrip(display.get()) < 0) {
        return 1;
    }
    const auto interface = registry.interface(KWayland::Client::Registry::Interface::FakeInput);
    if (!interface.name) {
        std::fputs("The QA compositor does not provide KWin FakeInput.\n", stderr);
        return 1;
    }

    std::unique_ptr<KWayland::Client::FakeInput> input(
        registry.createFakeInput(interface.name, interface.version));
    input->authenticate(QStringLiteral("Status Display QA"), QStringLiteral("Drive an isolated test window"));
    if (action == "refresh" && argc == 2) {
        input->requestKeyboardKeyPress(KEY_R);
        input->requestKeyboardKeyRelease(KEY_R);
    } else if ((action == "move" || action == "click") && argc == 4) {
        input->requestPointerMoveAbsolute(QPointF(std::strtol(argv[2], nullptr, 10),
                                                  std::strtol(argv[3], nullptr, 10)));
        if (action == "click") {
            input->requestPointerButtonClick(BTN_LEFT);
        }
    } else {
        return 2;
    }
    return wl_display_roundtrip(display.get()) < 0 ? 1 : 0;
}
