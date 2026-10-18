use super::Language;
pub(super) fn manual(l: Language) -> [(&'static str, &'static str); 11] {
    [
        (l.tr("Entrada", "Input"), l.tr("Bajo y Guitarra usan el canal 1. Instrumento usa el canal 2. Elige la entrada que mejor acompa\u{f1}e tu instrumento.", "Bass and Guitar use channel 1. Instrument uses channel 2. Choose the input that suits your instrument.")),
        (l.tr("Volumen 1 / 2", "Volume 1 / 2"), l.tr("Ajusta el volumen de cada canal. Al subirlo puedes conseguir un sonido m\u{e1}s saturado.", "Adjust each channel's volume. Turn it up for a more saturated sound.")),
        (l.tr("Agudos 1 / 2", "Treble 1 / 2"), l.tr("A\u{f1}ade brillo o suaviza los agudos.", "Add brightness or soften the high frequencies.")),
        (l.tr("Graves 1 / 2", "Bass 1 / 2"), l.tr("Aumenta o reduce los graves.", "Boost or reduce the low frequencies.")),
        (l.tr("Bocina 18\" / 15\"", "Speaker 18\" / 15\""), l.tr("Prueba los dos perfiles de bocina y elige tu sonido favorito. Son aproximaciones del car\u{e1}cter de cada bocina.", "Try both speaker profiles and choose your favorite sound. Each profile approximates the character of that speaker.")),
        (l.tr("CAB S\u{cd} / NO", "CAB ON / OFF"), l.tr("Activa el sonido de la bocina o escucha directamente el amplificador.", "Turn the speaker sound on, or listen directly to the amplifier.")),
        (l.tr("Entrada / Salida", "Input / Output"), l.tr("Ajusta el nivel que entra al amplificador y el volumen final. Baja la salida si el sonido resulta demasiado fuerte.", "Adjust the level going into the amplifier and the final volume. Lower the output if the sound is too loud.")),
        (l.tr("Sin efecto", "Bypass"), l.tr("Escucha la se\u{f1}al sin el efecto para comparar.", "Listen without the effect to compare.")),
        (l.tr("Ajustes", "Settings"), l.tr("Calidad permite elegir entre menor consumo y mayor precisi\u{f3}n del sonido: 1\u{d7}, 2\u{d7} o 4\u{d7}. La carga alternativa es experimental y puede mantenerse en el modo normal.", "Quality offers a choice between lower resource use and greater sound accuracy: 1\u{d7}, 2\u{d7}, or 4\u{d7}. The alternative load is experimental and may remain in normal mode.")),
        (l.tr("C\u{f3}mo ajustar", "Adjusting knobs"), l.tr("Arrastra las perillas hacia arriba o abajo. Mant\u{e9}n Shift para ajustar con precisi\u{f3}n. Haz doble clic para restablecer el valor. Tambi\u{e9}n puedes usar la rueda del rat\u{f3}n.", "Drag knobs up or down. Hold Shift for fine adjustment. Double-click to reset. You can also use the mouse wheel.")),
        (l.tr("Editar porcentajes", "Editing percentages"), l.tr("Haz clic en el porcentaje y escribe un valor entre 0 y 100. Pulsa Enter para confirmar o Escape para cancelar. Puedes usar punto o coma decimal.", "Click a percentage and type a value from 0 to 100. Press Enter to apply or Escape to cancel. Both decimal points and commas are accepted.")),
    ]
}
