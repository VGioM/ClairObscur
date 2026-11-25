# ClairObscur pour Linux (KDE Plasma)

Choisissez l'heure à laquelle KDE Plasma passe du thème global clair au thème
sombre, et inversement, au lieu du lever et du coucher du soleil. Application
native (Rust, egui) livrée en AppImage.

## Utilisation

1. Cochez **Passer seul du thème clair au thème sombre**.
2. Choisissez **Aux heures que je choisis**, puis réglez **Clair à** et
   **Sombre à** (glissez sur les chiffres ou cliquez pour les taper).
3. Choisissez les thèmes globaux du jour et de la nuit, puis **Enregistrer**.

Si l'heure d'un passage est déjà dépassée (sombre à 17:00 alors qu'il est
18:00), le bon thème est appliqué tout de suite. Les boutons **Clair** et
**Sombre** forcent un thème jusqu'au prochain passage.

Rien ne tourne en arrière-plan : c'est Plasma qui bascule, l'appli ne règle que
son horaire et peut être fermée.

## Comment ça marche

- `~/.config/knighttimerc` : `[General] Source=Times` (ou `Automatic` pour le
  soleil), `[Times] SunriseStart` / `SunsetStart` au format `HH:mm`,
  `TransitionDuration` en secondes (10). Le démon `plasma-knighttimed` est
  redémarré pour qu'il relise le fichier.
- `~/.config/kdeglobals`, groupe `[KDE]` : `AutomaticLookAndFeel`,
  `DefaultLightLookAndFeel`, `DefaultDarkLookAndFeel`.
- Plasma change de thème au milieu de la transition, donc à HH:MM:05.

## Construire

```bash
./build-appimage.sh     # dans la distrobox « rustbuild » (Fedora 44)
./release-linux.sh      # construit et publie l'AppImage dans les releases
```
