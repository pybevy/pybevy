# Complete games

Six small, complete games — setup, input, game state, win/lose — each in one
file, using only documented public API. Run any of them from the repo root:

```bash
python examples/games/breakout.py
```

| Game | Controls | Demonstrates |
|---|---|---|
| `move_the_hero.py` | arrows / WASD | the minimal moving-sprite starter |
| `breakout.py` | Left/Right | collision + response, game-state resource, HUD, win/lose |
| `tetris.py` | arrows + Space | 200 sprites live-recolored per frame (`Mut[Sprite]`), edge-triggered input |
| `survivors.py` | arrows / WASD | spawn/despawn churn, View column DSL movement, NumPy-snapshot collision |
| `fps_arena.py` | W/S + turn, click | 3D, billboard enemies, first-person camera, `Res[MouseInput]` hitscan, UI over 3D, audio |
| `voxel.py` | W/S + turn, click/Space | runtime 3D entity churn, Python ray-march over a voxel grid |

Every game supports two env vars for demos and CI:

- `BENCH=1` — the game plays itself (deterministic auto-play)
- `PYBEVY_MAX_FRAMES=N` — auto-exit after N frames with a one-line result

```bash
BENCH=1 PYBEVY_MAX_FRAMES=600 python examples/games/survivors.py
```

Assets: the three files in `assets/games/` are CC0 by [Kenney](https://kenney.nl)
— full attribution and source links in [`assets/games/CREDITS.md`](../../assets/games/CREDITS.md).
