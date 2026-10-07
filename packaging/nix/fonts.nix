# The fonts the shell's `Theme` loads, under the names it looks for: Inter
# for text, and Material Symbols Rounded for icons. mochid finds them
# through `MOCHI_FONTS`, which the package and the dev shell set.
{
  runCommand,
  inter,
  material-symbols,
}:

runCommand "mochi-fonts" { } ''
  mkdir $out
  ln -s ${inter}/share/fonts/truetype/InterVariable.ttf $out/InterVariable.ttf
  ln -s "${material-symbols}/share/fonts/truetype/MaterialSymbolsRounded[FILL,GRAD,opsz,wght].ttf" \
    $out/MaterialSymbolsRounded.ttf
''
