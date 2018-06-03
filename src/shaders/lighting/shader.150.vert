#version 150 core

in vec2 a_CornerZeroToOne;

uniform PropertiesStatic {
    vec2 u_WindowSizeInPixels;
};

out vec2 v_TexCoord;
out vec2 v_PixelCoord;

void main() {
    vec2 screen_coord = vec2(
        a_CornerZeroToOne.x * 2 - 1,
        1 - a_CornerZeroToOne.y * 2);

    v_TexCoord = a_CornerZeroToOne;
    v_PixelCoord = a_CornerZeroToOne * u_WindowSizeInPixels;

    gl_Position = vec4(screen_coord, 0, 1);
}
