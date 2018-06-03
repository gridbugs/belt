#version 150 core

in vec2 a_CornerZeroToOne;

uniform Properties {
    vec2 u_OutputSizeInPixels;
};

out vec2 v_TexCoord;

void main() {
    vec2 screen_coord = vec2(
        a_CornerZeroToOne.x * 2 - 1,
        a_CornerZeroToOne.y * 2 - 1);

    v_TexCoord = a_CornerZeroToOne;

    gl_Position = vec4(screen_coord, 0, 1);
}
