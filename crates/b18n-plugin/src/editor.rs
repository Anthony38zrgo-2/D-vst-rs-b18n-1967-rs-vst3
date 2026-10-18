//! Illustrated editor. All parameter writes go through the host's gesture API.
use crate::params::{B18nParams, ChannelParam, LoadModelParam, SpeakerParam};
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui::*, widgets::ParamSlider};
use std::sync::Arc;

const BLUE: Color32 = Color32::from_rgb(159, 192, 213);
const INK: Color32 = Color32::from_rgb(20, 43, 65);
const ORANGE: Color32 = Color32::from_rgb(255, 193, 150);
const CONTROL_SCALE: f32 = 1.35;
const SMOKE: Color32 = Color32::from_rgb(245, 245, 245);
const LOWER_PANEL_TOP: f32 = 610.0;
pub(crate) const WIDTH: u32 = 1280;
pub(crate) const HEIGHT: u32 = 744;
// Scale the original 700px head and all its interactive regions together.
fn right_at(origin: Pos2, x: f32, y: f32) -> Pos2 {
    origin
        + vec2(
            310.0 + (x - 376.0) * CONTROL_SCALE,
            68.0 + (y - 100.0) * CONTROL_SCALE,
        )
}
fn brand_text(ui: &Ui, at: Pos2, text: &str, size: f32) {
    ui.painter().text(
        at,
        Align2::CENTER_CENTER,
        text,
        FontId::new(size, FontFamily::Name("Libron Bold".into())),
        Color32::WHITE,
    );
}
#[derive(Default)]
struct State {
    textures: Vec<TextureHandle>,
    manual: bool,
    settings: bool,
    last_speaker: Option<SpeakerParam>,
}
fn texture(ctx: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let decoded = image::load_from_memory(bytes)
        .expect("embedded UI image is valid")
        .to_rgba8();
    let size = [decoded.width() as usize, decoded.height() as usize];
    ctx.load_texture(
        name,
        ColorImage::from_rgba_unmultiplied(size, decoded.as_raw()),
        TextureOptions::LINEAR,
    )
}
fn egui_manual_button(texture: TextureId) -> ImageButton<'static> {
    ImageButton::new(
        Image::new((texture, vec2(64.0, 56.0)))
            .uv(Rect::from_min_max(pos2(0.10, 0.24), pos2(0.90, 0.77))),
    )
    .frame(false)
}
fn label(ui: &Ui, at: Pos2, text: &str, size: f32) {
    ui.painter().text(
        at,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(size * CONTROL_SCALE),
        INK,
    );
}
fn set<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}
// Circular textured mesh keeps the supplied illustration's blue corners out of the dial.
fn disc(
    ui: &Ui,
    texture: TextureId,
    center: Pos2,
    radius: f32,
    angle: f32,
    uv_center: Pos2,
    uv_radius: f32,
) {
    let mut mesh = Mesh::with_texture(texture);
    mesh.vertices.push(epaint::Vertex {
        pos: center,
        uv: uv_center,
        color: Color32::WHITE,
    });
    for i in 0..=64 {
        let t = i as f32 / 64.0 * std::f32::consts::TAU;
        mesh.vertices.push(epaint::Vertex {
            pos: center + vec2((t + angle).cos(), (t + angle).sin()) * radius,
            uv: uv_center + vec2(t.cos(), t.sin()) * uv_radius,
            color: Color32::WHITE,
        });
        if i > 0 {
            mesh.indices.extend_from_slice(&[0, i, i + 1]);
        }
    }
    ui.painter().add(Shape::mesh(mesh));
}
fn knob(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    tex: TextureId,
    center: Pos2,
    title: &str,
    id: usize,
) {
    let rect = Rect::from_center_size(center, vec2(60.0, 66.0) * CONTROL_SCALE);
    let response = ui.interact(rect, Id::new(("knob", id)), Sense::click_and_drag());
    if response.drag_started() {
        setter.begin_set_parameter(param);
    }
    if response.dragged() {
        let delta = ui.input(|i| -i.pointer.delta().y);
        let sensitivity = ui.input(|i| if i.modifiers.shift { 0.0006 } else { 0.004 });
        setter.set_parameter_normalized(
            param,
            (param.unmodulated_normalized_value() + delta * sensitivity).clamp(0.0, 1.0),
        );
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        set(setter, param, param.default_plain_value());
    }
    if response.hovered() {
        let wheel = ui.input(|i| i.raw_scroll_delta.y);
        if wheel != 0.0 {
            set(
                setter,
                param,
                (param.value() + wheel * 0.001).clamp(0.0, 1.0),
            );
        }
    }
    let value = param.modulated_normalized_value();
    let visual = ui
        .ctx()
        .animate_value_with_time(Id::new(("angle", id)), value, 0.035);
    for tick in 0..=10 {
        let a = (-225.0 + tick as f32 * 27.0).to_radians();
        let v = vec2(a.cos(), a.sin());
        ui.painter().line_segment(
            [
                center + v * 28.0 * CONTROL_SCALE,
                center + v * 31.0 * CONTROL_SCALE,
            ],
            Stroke::new(1.3, Color32::from_rgb(235, 230, 215)),
        );
    }
    disc(
        ui,
        tex,
        center,
        26.0 * CONTROL_SCALE,
        (-135.0 + visual * 270.0).to_radians(),
        pos2(0.5, 0.505),
        0.195,
    );
    label(ui, center + vec2(0.0, 58.0) * CONTROL_SCALE, title, 12.0);
    label(
        ui,
        center + vec2(0.0, 72.0) * CONTROL_SCALE,
        &format!("{:.0}%", param.value() * 100.0),
        9.5,
    );
    response.on_hover_text("Arrastra para ajustar · Shift: ajuste fino · Doble clic: restablecer");
}

fn setup(ctx: &Context, state: &mut State) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Libron".into(),
        FontData::from_static(include_bytes!("../../../assets/ui/Libron-Regular.ttf")).into(),
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Libron".into());
    fonts.font_data.insert(
        "Libron Bold".into(),
        FontData::from_static(include_bytes!("../../../assets/ui/Libron-Bold.ttf")).into(),
    );
    fonts.families.insert(
        FontFamily::Name("Libron Bold".into()),
        vec!["Libron Bold".into()],
    );
    ctx.set_fonts(fonts);
    ctx.set_visuals(Visuals::light());
    ctx.style_mut(|style| {
        style.visuals.override_text_color = Some(INK);
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(17.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(17.0));
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(23.0, FontFamily::Name("Libron Bold".into())),
        );
        style.spacing.button_padding = vec2(8.0, 5.0);
    });
    state.textures = vec![
        texture(
            ctx,
            "cabinet",
            include_bytes!("../../../assets/ui/cabinet-cutout.png"),
        ),
        texture(ctx, "head", include_bytes!("../../../assets/ui/head.jpg")),
        texture(ctx, "knob", include_bytes!("../../../assets/ui/knob.jpg")),
        texture(
            ctx,
            "toggle",
            include_bytes!("../../../assets/ui/toggle.jpg"),
        ),
        texture(
            ctx,
            "manual",
            include_bytes!("../../../assets/ui/manual.png"),
        ),
        texture(
            ctx,
            "rust-pluggins-emblem",
            include_bytes!("../../../assets/ui/rust-pluggins-emblem.png"),
        ),
        texture(
            ctx,
            "connected-jack",
            include_bytes!("../../../assets/ui/connected-jack.png"),
        ),
    ];
}

/// Brackets and routing are drawn separately so the crossing has no junction.
fn channel_guides(ui: &Ui, origin: Pos2) {
    let painter = ui.painter();
    let white = Stroke::new(1.4, Color32::WHITE);
    let point = |x, y| origin + vec2(x, y);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([point(a.0, a.1), point(b.0, b.1)], white)
    };
    for first in [562.0, 759.1] {
        let centers =
            [first, first + 65.7, first + 131.4].map(|x| right_at(origin, x, 374.0).x - origin.x);
        line((centers[0], 557.0), (centers[2], 557.0));
        for x in centers {
            line((x, 549.0), (x, 557.0));
        }
        line((centers[1], 557.0), (centers[1], 562.0));
    }
    let jack = |index: usize| right_at(origin, 423.0 + index as f32 * 43.5, 381.0).x - origin.x;
    // Input 1 and Input 2 merge only into the first channel.
    line((jack(0), 487.0), (jack(0), 502.0));
    line((jack(1), 487.0), (jack(1), 502.0));
    line((jack(0), 502.0), (jack(1), 502.0));
    let merge = (jack(0) + jack(1)) * 0.5;
    line((merge, 502.0), (merge, 557.0));
    // Leave a gap under INSTR's independent vertical route.
    line((merge, 557.0), (jack(2) - 8.0, 557.0));
    line(
        (jack(2) + 8.0, 557.0),
        (right_at(origin, 562.0, 374.0).x - origin.x, 557.0),
    );
    line((jack(2), 487.0), (jack(2), 593.0));
    line((jack(2), 593.0), (1045.0, 593.0));
    line((1045.0, 593.0), (1045.0, 557.0));
    line(
        (1045.0, 557.0),
        (right_at(origin, 890.5, 374.0).x - origin.x, 557.0),
    );
}

pub fn create(params: Arc<B18nParams>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        State::default(),
        |ctx, state| {
            setup(ctx, state);
        },
        move |ctx, setter, state| draw(ctx, setter, state, &params),
    )
}

fn draw(ctx: &Context, setter: &ParamSetter, state: &mut State, params: &B18nParams) {
    CentralPanel::default()
        .frame(Frame::NONE.fill(BLUE))
        .show(ctx, |ui| {
            let origin = ui.min_rect().min;
            let at = |x, y| right_at(origin, x, y);
            ui.painter().rect_filled(
                Rect::from_min_max(
                    origin + vec2(290.0, LOWER_PANEL_TOP),
                    origin + vec2(WIDTH as f32, HEIGHT as f32),
                ),
                0.0,
                SMOKE,
            );
            ui.painter().line_segment(
                [
                    origin + vec2(290.0, LOWER_PANEL_TOP),
                    origin + vec2(WIDTH as f32, LOWER_PANEL_TOP),
                ],
                Stroke::new(1.4, Color32::WHITE),
            );
            ui.painter().rect_filled(
                Rect::from_min_size(origin, vec2(290.0, HEIGHT as f32)),
                0.0,
                ORANGE,
            );
            brand_text(ui, origin + vec2(145.0, 52.0), "B18N", 52.0);
            ui.painter().image(
                state.textures[5].id(),
                Rect::from_min_size(origin + vec2(101.0, 550.0), vec2(88.0, 88.0)),
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            brand_text(ui, origin + vec2(145.0, 670.0), "Rust Pluggins", 30.0);
            ui.painter().image(
                state.textures[0].id(),
                Rect::from_min_size(origin + vec2(16.0, 105.0), vec2(258.0, 420.0)),
                Rect::from_min_max(pos2(0.07, 0.04), pos2(0.93, 0.96)),
                Color32::WHITE,
            );
            ui.painter().image(
                state.textures[1].id(),
                Rect::from_min_size(at(376.0, 100.0), vec2(700.0, 323.0) * CONTROL_SCALE),
                Rect::from_min_max(pos2(0.05, 0.10), pos2(0.95, 0.84)),
                Color32::WHITE,
            );
            channel_guides(ui, origin);
            for (index, value) in [
                ChannelParam::Bass,
                ChannelParam::Guitar,
                ChannelParam::Instrument,
            ]
            .into_iter()
            .enumerate()
            {
                let center = at(423.0 + index as f32 * 43.5, 381.0);
                let response = ui.interact(
                    Rect::from_center_size(center, vec2(34.0, 36.0) * CONTROL_SCALE),
                    Id::new(("jack", index)),
                    Sense::click(),
                );
                if response.clicked() {
                    set(setter, &params.channel, value);
                }
                if params.channel.value() == value {
                    ui.painter().image(
                        state.textures[6].id(),
                        Rect::from_min_size(center + vec2(-84.0, -18.0), vec2(95.0, 125.0)),
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.painter().circle_stroke(
                        center,
                        15.0 * CONTROL_SCALE,
                        Stroke::new(1.7, Color32::from_rgb(230, 198, 145)),
                    );
                }
                response.on_hover_text(match value {
                    ChannelParam::Bass => "Bass · Canal 1",
                    ChannelParam::Guitar => "Guitar · Canal 1",
                    ChannelParam::Instrument => "Instrument · Canal 2",
                });
            }
            let toggle_center = at(1004.0, 374.0);
            let active_cab = params.speaker.value() != SpeakerParam::Direct;
            let toggle_response = ui.interact(
                Rect::from_center_size(toggle_center, vec2(52.0, 60.0) * CONTROL_SCALE),
                Id::new("cab-toggle"),
                Sense::click(),
            );
            if toggle_response.clicked() {
                if active_cab {
                    state.last_speaker = Some(params.speaker.value());
                }
                set(
                    setter,
                    &params.speaker,
                    if active_cab {
                        SpeakerParam::Direct
                    } else {
                        state.last_speaker.unwrap_or(SpeakerParam::Eighteen)
                    },
                );
            }
            disc(
                ui,
                state.textures[3].id(),
                toggle_center,
                16.0 * CONTROL_SCALE,
                0.0,
                pos2(0.495, 0.535),
                0.17,
            );
            let tilt = ui
                .ctx()
                .animate_bool_with_time(Id::new("cab-tilt"), active_cab, 0.12);
            let mut lever = Mesh::with_texture(state.textures[3].id());
            let rotation = emath::Rot2::from_angle(tilt * std::f32::consts::PI);
            for (offset, uv) in [
                (vec2(-6.0, 0.0), pos2(0.455, 0.52)),
                (vec2(6.0, 0.0), pos2(0.57, 0.52)),
                (vec2(6.0, 29.0), pos2(0.57, 0.815)),
                (vec2(-6.0, 29.0), pos2(0.455, 0.815)),
            ] {
                lever.vertices.push(epaint::Vertex {
                    pos: toggle_center + rotation * offset * CONTROL_SCALE,
                    uv,
                    color: Color32::WHITE,
                });
            }
            lever.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            ui.painter().add(Shape::mesh(lever));
            label(
                ui,
                at(1004.0, 432.0),
                if active_cab { "CAB ON" } else { "CAB OFF" },
                11.0,
            );
            toggle_response.on_hover_text("Activar o desactivar la bocina");
            // Controls below the artwork stay large and readable at the native editor size.
            let knobs = [
                (&params.volume_1, "VOLUME"),
                (&params.treble_1, "TREBLE"),
                (&params.bass_1, "BASS"),
                (&params.volume_2, "VOLUME"),
                (&params.treble_2, "TREBLE"),
                (&params.bass_2, "BASS"),
            ];
            for (i, (param, title)) in knobs.iter().enumerate() {
                knob(
                    ui,
                    setter,
                    param,
                    state.textures[2].id(),
                    at(562.0 + i as f32 * 65.7, 374.0),
                    title,
                    i,
                );
            }
            label(ui, at(628.0, 474.0), "CANAL 1", 11.0);
            label(ui, at(825.0, 474.0), "CANAL 2", 11.0);
            ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_min_size(
                    at(385.0, 500.0) + vec2(0.0, 22.0),
                    vec2(680.0, 85.0) * CONTROL_SCALE,
                )),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Entrada");
                        for (value, name) in [
                            (ChannelParam::Bass, "Bass"),
                            (ChannelParam::Guitar, "Guitar"),
                            (ChannelParam::Instrument, "Instrument"),
                        ] {
                            if ui
                                .selectable_label(params.channel.value() == value, name)
                                .clicked()
                            {
                                set(setter, &params.channel, value);
                            }
                        }
                        ui.separator();
                        ui.label("Bocina");
                        let active = params.speaker.value();
                        if active != SpeakerParam::Direct {
                            state.last_speaker = Some(active);
                        }
                        for (value, name) in [
                            (SpeakerParam::Eighteen, "18\""),
                            (SpeakerParam::Fifteen, "15\""),
                        ] {
                            if ui
                                .selectable_label(active == value, name)
                                .on_hover_text("Perfil de bocina aproximado")
                                .clicked()
                            {
                                set(setter, &params.speaker, value);
                            }
                        }
                        if ui
                            .button(if active == SpeakerParam::Direct {
                                "CAB OFF"
                            } else {
                                "CAB ON"
                            })
                            .clicked()
                        {
                            set(
                                setter,
                                &params.speaker,
                                if active == SpeakerParam::Direct {
                                    state.last_speaker.unwrap_or(SpeakerParam::Eighteen)
                                } else {
                                    SpeakerParam::Direct
                                },
                            );
                        }
                    });
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.label("Entrada");
                        ui.add(
                            ParamSlider::for_param(&params.input_trim_db, setter).with_width(170.0),
                        );
                        ui.label("Salida");
                        ui.add(
                            ParamSlider::for_param(&params.output_trim_db, setter)
                                .with_width(170.0),
                        );
                        if ui
                            .selectable_label(params.bypass.value(), "Bypass")
                            .clicked()
                        {
                            set(setter, &params.bypass, !params.bypass.value());
                        }
                        if ui.button("Ajustes").clicked() {
                            state.settings = !state.settings;
                        }
                    });
                },
            );
            ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_min_size(
                    origin + vec2(1190.0, 12.0),
                    vec2(80.0, 65.0),
                )),
                |ui| {
                    if ui
                        .add(egui_manual_button(state.textures[4].id()))
                        .on_hover_text("Manual · Guía de uso")
                        .clicked()
                    {
                        state.manual = !state.manual;
                    }
                },
            );
        });
    if state.settings {
        Window::new("Ajustes")
            .open(&mut state.settings)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Calidad: menor consumo o mayor precisión del sonido.");
                ui.add(ParamSlider::for_param(
                    &params.advanced.oversampling,
                    setter,
                ));
                ui.label("Carga del amplificador");
                ui.horizontal(|ui| {
                    for (value, name) in [
                        (LoadModelParam::Nominal, "Normal"),
                        (LoadModelParam::RlcEstimated, "Alternativa"),
                    ] {
                        if ui
                            .selectable_label(params.advanced.load_model.value() == value, name)
                            .clicked()
                        {
                            set(setter, &params.advanced.load_model, value);
                        }
                    }
                });
                ui.label("La carga alternativa es experimental y puede usar el modo normal.");
                ui.label("Las bocinas de 18\" y 15\" son perfiles aproximados.");
                ui.separator();
                ui.hyperlink_to(
                    "Tipografía Libron v0.31",
                    "https://github.com/nicoverbruggen/libron/releases/tag/v0.31",
                );
                ui.collapsing("Licencia de la tipografía", |ui| {
                    ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                        ui.label(include_str!("../../../assets/ui/Libron-LICENSE.txt"));
                    });
                });
            });
    }
    if state.manual {
        Window::new("Manual · B18N").open(&mut state.manual).default_pos(pos2(690.0, 105.0)).default_width(500.0).resizable(false).collapsible(false).show(ctx, |ui| {
                ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                    for (title, body) in [
                        ("Entrada", "Bass y Guitar usan el canal 1. Instrument usa el canal 2. Selecciona la entrada que mejor acompañe tu instrumento."),
                        ("Volume 1 / 2", "Ajusta el volumen de cada canal. Al subirlo puedes conseguir un sonido más saturado."),
                        ("Treble 1 / 2", "Añade brillo o suaviza los agudos."),
                        ("Bass 1 / 2", "Aumenta o reduce los graves."),
                        ("Bocina 18\" / 15\"", "Prueba los dos perfiles de bocina y elige tu sonido favorito. Son aproximaciones del carácter de cada bocina."),
                        ("CAB ON / OFF", "Activa el sonido de la bocina o escucha directamente el amplificador."),
                        ("Entrada / Salida", "Ajusta cuánto nivel entra al amplificador y el volumen final. Baja la salida si el sonido resulta demasiado fuerte."),
                        ("Bypass", "Escucha la señal sin el efecto para comparar."),
                        ("Ajustes", "Calidad permite elegir entre menor consumo de recursos y mayor precisión del sonido: 1×, 2× o 4×. La carga alternativa es experimental y puede mantenerse en el modo normal."),
                        ("Cómo ajustar", "Arrastra las perillas hacia arriba o abajo. Mantén Shift para ajustar con precisión. Haz doble clic para recuperar el valor inicial. También puedes usar la rueda del ratón sobre una perilla."),
                    ] { ui.strong(title); ui.label(body); ui.add_space(10.0); }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    struct Host(Mutex<Vec<String>>);
    impl GuiContext for Host {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Vst3
        }
        fn request_resize(&self) -> bool {
            true
        }
        unsafe fn raw_begin_set_parameter(&self, p: ParamPtr) {
            self.0
                .lock()
                .unwrap()
                .push(format!("begin:{}", unsafe { p.name() }));
        }
        unsafe fn raw_set_parameter_normalized(&self, p: ParamPtr, value: f32) {
            self.0
                .lock()
                .unwrap()
                .push(format!("set:{}:{value:.3}", unsafe { p.name() }));
        }
        unsafe fn raw_end_set_parameter(&self, p: ParamPtr) {
            self.0
                .lock()
                .unwrap()
                .push(format!("end:{}", unsafe { p.name() }));
        }
        fn get_state(&self) -> PluginState {
            unimplemented!("not used by editor")
        }
        fn set_state(&self, _: PluginState) {
            unimplemented!("not used by editor")
        }
    }
    fn frame(
        ctx: &Context,
        state: &mut State,
        host: &Host,
        params: &B18nParams,
        events: Vec<Event>,
    ) -> FullOutput {
        ctx.run(
            RawInput {
                time: Some(ctx.cumulative_pass_nr() as f64 * 0.25),
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    vec2(WIDTH as f32, HEIGHT as f32),
                )),
                events,
                ..Default::default()
            },
            |ctx| draw(ctx, &ParamSetter::new(host), state, params),
        )
    }
    fn click(ctx: &Context, state: &mut State, host: &Host, params: &B18nParams, point: Pos2) {
        frame(
            ctx,
            state,
            host,
            params,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            state,
            host,
            params,
            vec![Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn manual_and_cabinet_toggle_work_and_notify_host() {
        let ctx = Context::default();
        let mut state = State::default();
        setup(&ctx, &mut state);
        let host = Host::default();
        let params = B18nParams::default();
        frame(&ctx, &mut state, &host, &params, vec![]);
        click(&ctx, &mut state, &host, &params, pos2(1220.0, 42.0));
        assert!(state.manual, "manual icon opens the guide");
        state.manual = false;
        frame(&ctx, &mut state, &host, &params, vec![]);
        click(
            &ctx,
            &mut state,
            &host,
            &params,
            right_at(Pos2::ZERO, 1004.0, 374.0),
        );
        assert_eq!(
            *host.0.lock().unwrap(),
            vec!["begin:Speaker", "set:Speaker:0.500", "end:Speaker"]
        );
    }
    #[test]
    fn knob_drag_is_one_host_automation_gesture() {
        let ctx = Context::default();
        let mut state = State::default();
        setup(&ctx, &mut state);
        let host = Host::default();
        let params = B18nParams::default();
        frame(&ctx, &mut state, &host, &params, vec![]);
        let start = right_at(Pos2::ZERO, 562.0, 374.0);
        let end = right_at(Pos2::ZERO, 562.0, 334.0);
        frame(
            &ctx,
            &mut state,
            &host,
            &params,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            &ctx,
            &mut state,
            &host,
            &params,
            vec![Event::PointerMoved(end)],
        );
        frame(
            &ctx,
            &mut state,
            &host,
            &params,
            vec![Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let events = host.0.lock().unwrap();
        assert_eq!(events.first().unwrap(), "begin:Volume 1");
        assert!(events
            .iter()
            .any(|event| event.starts_with("set:Volume 1:")));
        assert_eq!(events.last().unwrap(), "end:Volume 1");
        assert_eq!(
            events
                .iter()
                .filter(|event| event.starts_with("begin:"))
                .count(),
            1
        );
    }
    #[test]
    fn input_jacks_select_the_correct_host_channel() {
        for index in 0..3 {
            let ctx = Context::default();
            let mut state = State::default();
            setup(&ctx, &mut state);
            let host = Host::default();
            let params = B18nParams::default();
            frame(&ctx, &mut state, &host, &params, vec![]);
            click(
                &ctx,
                &mut state,
                &host,
                &params,
                right_at(Pos2::ZERO, 423.0 + index as f32 * 43.5, 381.0),
            );
            assert_eq!(
                *host.0.lock().unwrap(),
                vec![
                    "begin:Channel".to_string(),
                    format!("set:Channel:{:.3}", index as f32 / 2.0),
                    "end:Channel".to_string()
                ]
            );
        }
    }
    /// Rasterizes the actual egui meshes for review without needing a DAW or display server.
    #[test]
    fn render_editor_preview() {
        let ctx = Context::default();
        let mut state = State::default();
        setup(&ctx, &mut state);
        state.manual = std::env::var_os("B18N_UI_PREVIEW_MANUAL").is_some();
        let host = Host::default();
        let selected = match std::env::var("B18N_UI_PREVIEW_INPUT").as_deref() {
            Ok("2") => ChannelParam::Guitar,
            Ok("instr") => ChannelParam::Instrument,
            _ => ChannelParam::Bass,
        };
        let params = B18nParams {
            channel: EnumParam::new("Channel", selected),
            ..Default::default()
        };
        let first = frame(&ctx, &mut state, &host, &params, vec![]);
        let mut textures = std::collections::HashMap::new();
        fn update(
            textures: &mut std::collections::HashMap<TextureId, ColorImage>,
            output: &FullOutput,
        ) {
            for (id, delta) in &output.textures_delta.set {
                let image = match &delta.image {
                    ImageData::Color(image) => (**image).clone(),
                    ImageData::Font(font) => ColorImage {
                        size: font.size,
                        pixels: font.srgba_pixels(None).collect(),
                    },
                };
                if let Some([x, y]) = delta.pos {
                    let target = textures.get_mut(id).unwrap();
                    for row in 0..image.height() {
                        for col in 0..image.width() {
                            target[(x + col, y + row)] = image[(col, row)];
                        }
                    }
                } else {
                    textures.insert(*id, image);
                }
            }
        }
        update(&mut textures, &first);
        let second = frame(&ctx, &mut state, &host, &params, vec![]);
        update(&mut textures, &second);
        let primitives = ctx.tessellate(second.shapes, second.pixels_per_point);
        assert!(!primitives.is_empty());
        let Some(folder) = std::env::var_os("B18N_UI_PREVIEW") else {
            return;
        };
        let mut canvas = image::RgbaImage::new(WIDTH, HEIGHT);
        for primitive in primitives {
            let epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                continue;
            };
            let Some(texture) = textures.get(&mesh.texture_id) else {
                continue;
            };
            for tri in mesh.indices.chunks_exact(3) {
                let a = mesh.vertices[tri[0] as usize];
                let b = mesh.vertices[tri[1] as usize];
                let c = mesh.vertices[tri[2] as usize];
                let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
                let area = cross(b.pos - a.pos, c.pos - a.pos);
                if area.abs() < 1e-6 {
                    continue;
                }
                let bounds =
                    Rect::from_min_max(a.pos.min(b.pos).min(c.pos), a.pos.max(b.pos).max(c.pos))
                        .intersect(primitive.clip_rect)
                        .intersect(Rect::from_min_size(
                            Pos2::ZERO,
                            vec2(WIDTH as f32, HEIGHT as f32),
                        ));
                for y in bounds.min.y.max(0.0) as u32..bounds.max.y.ceil().min(HEIGHT as f32) as u32
                {
                    for x in
                        bounds.min.x.max(0.0) as u32..bounds.max.x.ceil().min(WIDTH as f32) as u32
                    {
                        let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let wa = cross(b.pos - p, c.pos - p) / area;
                        let wb = cross(c.pos - p, a.pos - p) / area;
                        let wc = 1.0 - wa - wb;
                        if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                            continue;
                        }
                        let uv = a.uv.to_vec2() * wa + b.uv.to_vec2() * wb + c.uv.to_vec2() * wc;
                        let tx = (uv.x * texture.width() as f32)
                            .clamp(0.0, (texture.width() - 1) as f32)
                            as usize;
                        let ty = (uv.y * texture.height() as f32)
                            .clamp(0.0, (texture.height() - 1) as f32)
                            as usize;
                        let texel = texture[(tx, ty)].to_array();
                        let ca = a.color.to_array();
                        let cb = b.color.to_array();
                        let cc = c.color.to_array();
                        let mut src = [0.0; 4];
                        for i in 0..4 {
                            src[i] = texel[i] as f32 / 255.0
                                * (ca[i] as f32 * wa + cb[i] as f32 * wb + cc[i] as f32 * wc)
                                / 255.0;
                        }
                        let dst = canvas.get_pixel_mut(x, y);
                        for i in 0..3 {
                            dst[i] = ((src[i] + dst[i] as f32 / 255.0 * (1.0 - src[3])) * 255.0)
                                .clamp(0.0, 255.0) as u8;
                        }
                        dst[3] = 255;
                    }
                }
            }
        }
        std::fs::create_dir_all(&folder).unwrap();
        canvas
            .save(std::path::Path::new(&folder).join(if state.manual {
                "b18n-manual.png"
            } else {
                match selected {
                    ChannelParam::Bass => "b18n-editor.png",
                    ChannelParam::Guitar => "b18n-input-2.png",
                    ChannelParam::Instrument => "b18n-instr.png",
                }
            }))
            .unwrap();
    }
}
