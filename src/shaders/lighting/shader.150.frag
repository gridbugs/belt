#version 150 core

in vec2 v_TexCoord;
out vec4 Target0;
uniform sampler2D t_Colour;
uniform sampler2D t_Visibility;

void main() {
    vec4 colour = texture(t_Colour, v_TexCoord);
    Target0 = colour;
}
