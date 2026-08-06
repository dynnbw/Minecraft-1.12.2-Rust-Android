import sys

p = 'src/net/minecraft/client/Minecraft.rs'
lines = open(p, encoding='utf-8').read().split('\n')

def find(pat, start=0):
    for i in range(start, len(lines)):
        if pat in lines[i]:
            return i
    raise SystemExit("not found: " + pat)

# ---------- 1. imports ----------
imp_idx = find('    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},')
lines[imp_idx] = '    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent},'

# ---------- 2. field + init ----------
fld_idx = find('    pendingResizeSince: Option<Instant>,')
lines.insert(fld_idx + 1, '    #[cfg(target_os = "android")]')
lines.insert(fld_idx + 2, '    lastTouchPosition: Option<PhysicalPosition<f64>>,')

init_idx = find('            pendingResizeSince: None,')
lines.insert(init_idx + 1, '            #[cfg(target_os = "android")]')
lines.insert(init_idx + 2, '            lastTouchPosition: None,')

# ---------- 3. extract branches ----------
cur_start = find('            WindowEvent::CursorMoved { position, .. } => {')
cur_end   = find('            WindowEvent::CursorEntered { .. } => {')
press_start = find('            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {')
press_end   = find('            WindowEvent::MouseInput { state: ElementState::Released, button, .. } => {')
rel_start   = press_end
rel_end     = find('            WindowEvent::Touch(touch) => {')

def body(start, end):
    out = []
    for l in lines[start+1:end-1]:
        out.append(l[8:] if l.startswith('                ') else l)
    return out

press_body = body(press_start, press_end)
press_body = [l.replace('eventLoop.exit(); return;', 'eventLoop.exit(); return true;') for l in press_body]
press_body = [l.replace('return;', 'return false;') for l in press_body]
rel_body = body(rel_start, rel_end)
cur_body = body(cur_start, cur_end)

methods = [
    '    // Mouse/cursor handling shared by desktop MouseInput/CursorMoved and the',
    '    // Android touch bridge (Android converts physical-mouse clicks into',
    '    // Touchscreen motion events; touches also map to right-click semantics).',
    '    fn handleCursorMove(',
    '        &mut self,',
    '        position: PhysicalPosition<f64>,',
    '        eventLoop: &ActiveEventLoop,',
    '        fatalError: &mut Option<anyhow::Error>,',
    '    ) {',
]
for l in cur_body:
    methods.append('        ' + l if l.strip() else l)
methods += [
    '    }',
    '',
    '    fn handleMousePress(',
    '        &mut self,',
    '        eventLoop: &ActiveEventLoop,',
    '        button: MouseButton,',
    '        fatalError: &mut Option<anyhow::Error>,',
    '    ) -> bool {',
]
for l in press_body:
    methods.append('        ' + l if l.strip() else l)
methods += [
    '        false',
    '    }',
    '',
    '    fn handleMouseRelease(&mut self, button: MouseButton) {',
]
for l in rel_body:
    methods.append('        ' + l if l.strip() else l)
methods += [
    '    }',
    '',
    '    /// First-person look shared by desktop DeviceEvent::MouseMotion and the',
    '    /// Android touch bridge (touch drag = mouse movement).',
    '    fn applyMouseMotion(&mut self, deltaX: f64, deltaY: f64) {',
    '        if !self.worldMouseGrabbed',
    '            || self.mainMenu.as_ref().is_some_and(MainMenuRuntime::isModalWorldGuiOpen)',
    '        {',
    '            return;',
    '        }',
    '        let turned = match (self.mainMenu.as_mut(), self.minecraft.as_ref()) {',
    '            (Some(runtime), Some(minecraft)) => runtime.turnPlayer(deltaX, deltaY, &minecraft.gameSettings),',
    '            _ => false,',
    '        };',
    '        if turned {',
    '            self.requestRedraw();',
    '        }',
    '    }',
]

anchor = find('    fn device_event(&mut self')
lines[anchor:anchor] = methods

# ---------- 4. replace branches ----------
def replace_branch(start, end, replacement_lines):
    del lines[start:end]
    lines[start:start] = replacement_lines

replace_branch(cur_start, cur_end, [
    '            WindowEvent::CursorMoved { position, .. } => {',
    '                self.handleCursorMove(position, eventLoop, &mut fatalError);',
    '            }',
])
press_start = find('            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {')
press_end   = find('            WindowEvent::MouseInput { state: ElementState::Released, button, .. } => {')
replace_branch(press_start, press_end, [
    '            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {',
    '                if self.handleMousePress(eventLoop, button, &mut fatalError) { return; }',
    '            }',
])
press_start = find('            WindowEvent::MouseInput { state: ElementState::Released, button, .. } => {')
rel_end = find('            WindowEvent::Touch(touch) => {')
replace_branch(press_start, rel_end, [
    '            WindowEvent::MouseInput { state: ElementState::Released, button, .. } => {',
    '                self.handleMouseRelease(button);',
    '            }',
])

# ---------- 5. Touch bridge ----------
touch_start = find('            WindowEvent::Touch(touch) => {')
touch_end = find('            WindowEvent::KeyboardInput { event, .. } => {')
replace_branch(touch_start, touch_end, [
    '            WindowEvent::Touch(touch) => {',
    '                log::debug!("input: Touch {:?} at {:?}", touch.phase, touch.location);',
    '                #[cfg(target_os = "android")]',
    '                {',
    '                    // Android converts physical-mouse clicks into touchscreen',
    '                    // motion events; a single finger maps to the right mouse',
    '                    // button (use/place in world, right-click in GUIs), and',
    '                    // dragging moves the cursor and turns the first-person',
    '                    // camera like mouse movement.',
    '                    let position = touch.location;',
    '                    match touch.phase {',
    '                        TouchPhase::Started => {',
    '                            let inWorld = self.mainMenu.as_ref().is_some_and(MainMenuRuntime::isWorld);',
    '                            let grabbed = self.worldMouseGrabbed;',
    '                            let button = if inWorld && !grabbed { MouseButton::Left } else { MouseButton::Right };',
    '                            if self.handleMousePress(eventLoop, button, &mut fatalError) { return; }',
    '                            self.lastTouchPosition = Some(position);',
    '                        }',
    '                        TouchPhase::Moved => {',
    '                            self.handleCursorMove(position, eventLoop, &mut fatalError);',
    '                            if let Some(prev) = self.lastTouchPosition {',
    '                                self.applyMouseMotion(position.x - prev.x, position.y - prev.y);',
    '                            }',
    '                            self.lastTouchPosition = Some(position);',
    '                        }',
    '                        TouchPhase::Ended | TouchPhase::Cancelled => {',
    '                            self.handleMouseRelease(MouseButton::Right);',
    '                            self.lastTouchPosition = None;',
    '                        }',
    '                    }',
    '                }',
    '            }',
])

open(p, 'w', encoding='utf-8', newline='').write('\n'.join(lines))
print("touch bridge + method extraction complete")
