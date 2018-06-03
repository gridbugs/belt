#version 150 core

in vec2 v_SpriteSheetSampleCoord;
out vec4 TargetColour;
out vec4 TargetVisibility;
uniform sampler2D t_SpriteSheet;

void main() {
    vec4 sprite_sheet_sample_colour = texture(t_SpriteSheet, v_SpriteSheetSampleCoord);
    if (sprite_sheet_sample_colour.a < 0.001) {
        discard;
    }
    TargetColour = sprite_sheet_sample_colour;
    TargetVisibility = vec4(0,0,0,1);
}
