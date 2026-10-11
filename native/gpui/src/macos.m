#import <AppKit/AppKit.h>
#import <AVFoundation/AVFoundation.h>

// All entry points are called on GPUI's main thread.
static NSStatusItem *statusItem;
static NSMutableArray<AVAudioPlayer *> *players;
static NSInteger pendingAction;

@interface LPPingActions : NSObject
@end
@implementation LPPingActions
- (void)settings:(id)sender { pendingAction = 1; }
- (void)toggle:(id)sender { pendingAction = 2; }
- (void)quit:(id)sender { pendingAction = 3; }
@end
static LPPingActions *actions;

void lp_init_status(void) {
    actions = [LPPingActions new];
    statusItem = [[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
    statusItem.button.title = @"◉";
    statusItem.button.toolTip = @"lolPing Native";
    NSMenu *menu = [NSMenu new];
    NSArray *titles = @[@"打开 lolPing 设置", @"暂停 / 恢复", @"退出 lolPing"];
    SEL selectors[] = {@selector(settings:), @selector(toggle:), @selector(quit:)};
    for (NSUInteger i = 0; i < titles.count; ++i) {
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:titles[i] action:selectors[i] keyEquivalent:@""];
        item.target = actions;
        [menu addItem:item];
    }
    statusItem.menu = menu;
}

int lp_poll_action(void) { int action = (int)pendingAction; pendingAction = 0; return action; }
void lp_status_enabled(bool enabled) { statusItem.button.title = enabled ? @"◉" : @"◎"; }

void lp_configure_settings(void *view) {
    NSWindow *window = [(__bridge NSView *)view window];
    dispatch_async(dispatch_get_main_queue(), ^{
        window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
        window.titlebarAppearsTransparent = YES;
        window.backgroundColor = [NSColor colorWithRed:0.094 green:0.129 blue:0.169 alpha:1.0];
    });
}

// Smoke tests send events only to their own preview window, without global input injection.
void lp_simulate_settings_input(void *view, int kind, double x, double y) {
    NSView *nativeView = (__bridge NSView *)view;
    NSWindow *window = nativeView.window;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSEvent *event;
        if (kind == 4 || kind == 5) {
            NSString *key = kind == 4 ? @"\x1b" : @"z";
            event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
                modifierFlags:kind == 5 ? NSEventModifierFlagCommand : 0
                timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber
                context:nil characters:key charactersIgnoringModifiers:key isARepeat:NO
                keyCode:kind == 4 ? 53 : 6];
        } else {
            const NSEventType types[] = {NSEventTypeMouseMoved, NSEventTypeLeftMouseDown,
                NSEventTypeLeftMouseDragged, NSEventTypeLeftMouseUp};
            if (kind < 0 || kind > 3) return;
            NSPoint point = NSMakePoint(x, nativeView.isFlipped ? y : nativeView.bounds.size.height - y);
            point = [nativeView convertPoint:point toView:nil];
            event = [NSEvent mouseEventWithType:types[kind] location:point modifierFlags:0
                timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber
                context:nil eventNumber:0 clickCount:1 pressure:kind == 1 || kind == 2 ? 1.0 : 0.0];
        }
        [window sendEvent:event];
    });
}

void lp_configure_overlay(void *view, double x, double y, double width, double height) {
    NSWindow *window = [(__bridge NSView *)view window];
    // AppKit emits resize notifications synchronously. Run after GPUI releases its App borrow.
    dispatch_async(dispatch_get_main_queue(), ^{
    window.styleMask = NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel;
    window.opaque = NO;
    window.backgroundColor = NSColor.clearColor;
    window.hasShadow = NO;
    window.ignoresMouseEvents = YES;
    window.hidesOnDeactivate = NO;
    window.level = NSScreenSaverWindowLevel;
    window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorStationary |
        NSWindowCollectionBehaviorIgnoresCycle;
    if ([window isKindOfClass:NSPanel.class]) {
        ((NSPanel *)window).becomesKeyOnlyIfNeeded = YES;
    }
    // CG/GPUI coordinates start at the primary display's top left; AppKit uses bottom left.
    double primaryHeight = NSScreen.screens.firstObject.frame.size.height;
    [window setFrame:NSMakeRect(x, primaryHeight - y - height, width, height) display:YES];
    [window orderFrontRegardless];
    });
}

bool lp_play_sound(const char *path, float volume) {
    if (!players) players = [NSMutableArray new];
    NSIndexSet *finished = [players indexesOfObjectsPassingTest:^BOOL(AVAudioPlayer *p, NSUInteger i, BOOL *stop) {
        (void)i; (void)stop;
        return !p.playing;
    }];
    [players removeObjectsAtIndexes:finished];
    if (players.count >= 16) { [players.firstObject stop]; [players removeObjectAtIndex:0]; }
    NSError *error = nil;
    AVAudioPlayer *player = [[AVAudioPlayer alloc] initWithContentsOfURL:
        [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]] error:&error];
    if (!player) return false;
    player.volume = volume;
    [players addObject:player];
    return [player play];
}

void lp_hide_window(void *view) { [[(__bridge NSView *)view window] orderOut:nil]; }

bool lp_overlay_valid(void *view) {
    NSWindow *window = [(__bridge NSView *)view window];
    return window.ignoresMouseEvents && !window.opaque && !window.hasShadow &&
        !window.isKeyWindow && window.level == NSScreenSaverWindowLevel &&
        (window.styleMask & NSWindowStyleMaskNonactivatingPanel) &&
        (window.collectionBehavior & NSWindowCollectionBehaviorCanJoinAllSpaces) &&
        (window.collectionBehavior & NSWindowCollectionBehaviorFullScreenAuxiliary);
}

void lp_display_bounds(uint32_t display, double *out) {
    CGRect bounds = CGDisplayBounds(display);
    out[0] = bounds.origin.x;
    out[1] = bounds.origin.y;
    out[2] = bounds.size.width;
    out[3] = bounds.size.height;
}
