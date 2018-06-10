#version 150 core

in vec2 v_SpriteSheetSampleCoord;
flat in uint v_IsPlayer;
out vec4 TargetColour;
out vec4 TargetVisibility;
uniform sampler2D t_SpriteSheet;

void main() {
    if (v_IsPlayer == 0u) {
        vec4 sprite_sheet_sample_colour = texture(t_SpriteSheet, v_SpriteSheetSampleCoord);
        TargetColour = sprite_sheet_sample_colour;
    }
}
