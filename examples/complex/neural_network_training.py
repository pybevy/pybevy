import math
from collections.abc import Iterator
from dataclasses import dataclass, field

import numpy as np

from pybevy.prelude import *  # type: ignore

try:
    import torch  # type: ignore[import-not-found]
    import torch.nn as nn  # type: ignore[import-not-found]
    import torch.optim as optim  # type: ignore[import-not-found]
    import torchvision  # type: ignore[import-not-found,import-untyped]
    import torchvision.transforms as transforms  # type: ignore[import-not-found,import-untyped]
except ImportError:
    print("ERROR: PyTorch is required for this example. Install with: pip install torch torchvision")
    exit(1)

MNIST_IMG_SIZE = 28
NUM_OUTPUTS = 10
CONV1_OUT_CH = 8
CONV1_KERNEL_SIZE = 5
CONV2_OUT_CH = 16
CONV2_IN_CH = 8  # From Conv1
CONV2_KERNEL_SIZE = 3
FC1_OUT_FEATURES = 128


class MnistCNN(nn.Module):
    def __init__(self):
        super().__init__()
        # Layers we need hooks on or weights from
        self.conv1 = nn.Conv2d(
            1, CONV1_OUT_CH, kernel_size=CONV1_KERNEL_SIZE, stride=1, padding=2
        )
        self.relu1 = nn.ReLU()
        self.conv2 = nn.Conv2d(
            CONV1_OUT_CH,
            CONV2_OUT_CH,
            kernel_size=CONV2_KERNEL_SIZE,
            stride=1,
            padding=1,
        )
        self.relu2 = nn.ReLU()
        self.pool = nn.MaxPool2d(2, 2)

        self.fc1 = nn.Linear(CONV2_OUT_CH * 14 * 14, FC1_OUT_FEATURES)
        self.relu3 = nn.ReLU()
        self.fc2 = nn.Linear(FC1_OUT_FEATURES, NUM_OUTPUTS)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # x: [N, 1, 28, 28]
        x = self.relu1(self.conv1(x))  # Activation 1 (for viz)
        x_pooled1 = x

        x = self.relu2(self.conv2(x_pooled1))
        x_pooled2 = self.pool(
            x
        )  # Activation 2 (for viz, after pooling) [N, 16, 14, 14]

        x_flat = x_pooled2.view(-1, CONV2_OUT_CH * 14 * 14)

        x = self.relu3(self.fc1(x_flat))
        return self.fc2(x)  # Raw scores (logits)


@component
@dataclass
class InputCube(Component):
    index: int


@component
@dataclass
class OutputCube(Component):
    index: int


@component
@dataclass
class Conv1WeightCube(Component):
    filter_idx: int
    ky: int
    kx: int


@component
@dataclass
class Conv2WeightCube(Component):
    filter_idx: int
    in_channel_idx: int
    ky: int
    kx: int


@resource
@dataclass
class DetailedConnectionMesh(Resource):
    mesh_handle: Handle[Mesh]
    input_to_conv1_vert_range: tuple[int, int]
    conv1_to_conv2_vert_range: tuple[int, int]
    conv2_to_output_vert_range: tuple[int, int]


@resource
@dataclass
class CnnVizState(Resource):
    """'Mailbox' for CNN input, output, weights, and activations."""

    input_image: np.ndarray = field(
        default_factory=lambda: np.zeros(
            (MNIST_IMG_SIZE, MNIST_IMG_SIZE), dtype=np.float32
        )
    )
    output_activations: np.ndarray = field(
        default_factory=lambda: np.zeros(NUM_OUTPUTS, dtype=np.float32)
    )
    conv1_weights: np.ndarray = field(
        default_factory=lambda: np.zeros(
            (CONV1_OUT_CH, 1, CONV1_KERNEL_SIZE, CONV1_KERNEL_SIZE), dtype=np.float32
        )
    )
    conv2_weights: np.ndarray = field(
        default_factory=lambda: np.zeros(
            (CONV2_OUT_CH, CONV2_IN_CH, CONV2_KERNEL_SIZE, CONV2_KERNEL_SIZE),
            dtype=np.float32,
        )
    )
    conv1_activations: np.ndarray = field(
        default_factory=lambda: np.zeros(
            (CONV1_OUT_CH, MNIST_IMG_SIZE, MNIST_IMG_SIZE), dtype=np.float32
        )
    )
    conv2_pooled_activations: np.ndarray = field(
        default_factory=lambda: np.zeros((CONV2_OUT_CH, 14, 14), dtype=np.float32)
    )
    fc1_weights: np.ndarray = field(
        default_factory=lambda: np.zeros(
            (FC1_OUT_FEATURES, CONV2_OUT_CH * 14 * 14), dtype=np.float32
        )
    )


@resource
@dataclass
class ModelBundleCNN(Resource):
    model: MnistCNN
    criterion: nn.CrossEntropyLoss
    optimizer: optim.Adam
    data_loader_iter: Iterator


def create_mnist_loader() -> Iterator:
    print("Downloading MNIST...")
    transform = transforms.Compose(
        [transforms.ToTensor(), transforms.Normalize((0.5,), (0.5,))]
    )
    trainset = torchvision.datasets.MNIST(
        root="./data", train=True, download=True, transform=transform
    )
    trainloader = torch.utils.data.DataLoader(
        trainset, batch_size=64, shuffle=True, num_workers=2
    )
    print("MNIST download complete.")
    return iter(trainloader)


def normalize_to_rgb(img_data: np.ndarray) -> np.ndarray:
    max_abs = np.max(np.abs(img_data))
    norm_map = (
        (img_data + max_abs) / (2 * max_abs)
        if max_abs > 0
        else np.full(img_data.shape, 0.5)
    )
    return (norm_map * 255).astype(np.uint8)


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    images: ResMut[Assets[Image]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    cnn_model = MnistCNN()
    cnn_criterion = nn.CrossEntropyLoss()
    cnn_optimizer = optim.Adam(cnn_model.parameters(), lr=0.001)
    data_loader_iter = create_mnist_loader()
    viz_state = CnnVizState()  # Create instance for hooks

    def hook_relu1(module: nn.Module, input: tuple[torch.Tensor, ...], output: torch.Tensor) -> None:
        viz_state.conv1_activations = output.data[0].cpu().numpy()

    def hook_pool(module: nn.Module, input: tuple[torch.Tensor, ...], output: torch.Tensor) -> None:
        viz_state.conv2_pooled_activations = output.data[0].cpu().numpy()

    cnn_model.relu1.register_forward_hook(hook_relu1)
    cnn_model.pool.register_forward_hook(hook_pool)

    commands.insert_resource(viz_state)
    commands.insert_resource(
        ModelBundleCNN(cnn_model, cnn_criterion, cnn_optimizer, data_loader_iter)
    )

    cube_mesh_handle = meshes.add(Cuboid(1.0))
    input_mat = materials.add(
        StandardMaterial(base_color=Color.srgb(1.0, 1.0, 1.0), unlit=True)
    )
    output_mat = materials.add(
        StandardMaterial(base_color=Color.srgb(0.8, 1.0, 0.8), unlit=True)
    )
    weight_mat = materials.add(
        StandardMaterial(base_color=Color.srgb(0.9, 0.9, 0.9), unlit=True)
    )

    INPUT_X, OUTPUT_X = -10.0, 10.0
    CUBE_SIZE_IO, CUBE_GAP_IO = 0.3, 0.05
    GRID_SIZE_IN = MNIST_IMG_SIZE * (CUBE_SIZE_IO + CUBE_GAP_IO)
    input_positions = []
    for i in range(MNIST_IMG_SIZE * MNIST_IMG_SIZE):
        row, col = divmod(i, MNIST_IMG_SIZE)
        x = INPUT_X
        y = (row * (CUBE_SIZE_IO + CUBE_GAP_IO)) - GRID_SIZE_IN / 2.0
        z = (col * (CUBE_SIZE_IO + CUBE_GAP_IO)) - GRID_SIZE_IN / 2.0
        pos = np.array([x, y, z])
        input_positions.append(pos)
        commands.spawn(
            Mesh3d(cube_mesh_handle),
            MeshMaterial3d(input_mat),
            Transform.from_xyz(*pos).with_scale(Vec3.splat(0.0)),
            InputCube(i),
        )

    OUTPUT_Y_SPREAD = 4.0
    output_positions = []
    for i in range(NUM_OUTPUTS):
        x = OUTPUT_X
        y = (
            (i * OUTPUT_Y_SPREAD / (NUM_OUTPUTS - 1)) - OUTPUT_Y_SPREAD / 2.0
            if NUM_OUTPUTS > 1
            else 0.0
        )
        z = 0.0
        pos = np.array([x, y, z])
        output_positions.append(pos)
        commands.spawn(
            Mesh3d(cube_mesh_handle),
            MeshMaterial3d(output_mat),
            Transform.from_xyz(*pos).with_scale(Vec3.splat(0.1)),
            OutputCube(i),
        )

    WEIGHT_CUBE_SIZE, WEIGHT_CUBE_GAP, FILTER_GAP = 0.2, 0.05, 0.5
    CONV1_X, CONV1_GRID_COLS = -4.0, 3
    conv1_filter_centers = []
    conv1_weight_positions = {}
    fksz_y1 = CONV1_KERNEL_SIZE * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP)
    fksz_z1 = CONV1_KERNEL_SIZE * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP)
    tgh1 = math.ceil(CONV1_OUT_CH / CONV1_GRID_COLS) * (fksz_y1 + FILTER_GAP)
    tgw1 = CONV1_GRID_COLS * (fksz_z1 + FILTER_GAP)
    for f in range(CONV1_OUT_CH):
        fr, fc = divmod(f, CONV1_GRID_COLS)
        cy = (fr * (fksz_y1 + FILTER_GAP)) - tgh1 / 2.0
        cz = (fc * (fksz_z1 + FILTER_GAP)) - tgw1 / 2.0
        cp = np.array([CONV1_X, cy, cz])
        conv1_filter_centers.append(cp)
        for ky in range(CONV1_KERNEL_SIZE):
            for kx in range(CONV1_KERNEL_SIZE):
                x, y, z = (
                    CONV1_X,
                    cy + (ky * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP)) - fksz_y1 / 2.0,
                    cz + (kx * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP)) - fksz_z1 / 2.0,
                )
                pos = np.array([x, y, z])
                conv1_weight_positions[(f, ky, kx)] = pos  # Store position by index
                commands.spawn(
                    Mesh3d(cube_mesh_handle),
                    MeshMaterial3d(weight_mat),
                    Transform.from_xyz(*pos).with_scale(Vec3.splat(0.0)),
                    Conv1WeightCube(filter_idx=f, ky=ky, kx=kx),
                )

    CONV2_X, CONV2_GRID_COLS = 4.0, 4
    conv2_filter_centers = []
    conv2_weight_positions = {}
    fksz_y2 = CONV2_KERNEL_SIZE * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP)
    fksz_z2 = CONV2_KERNEL_SIZE * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP)
    tgh2 = math.ceil(CONV2_OUT_CH / CONV2_GRID_COLS) * (fksz_y2 + FILTER_GAP)
    tgw2 = CONV2_GRID_COLS * (fksz_z2 + FILTER_GAP)
    # Store positions for ALL input channels now, even if we only spawn cubes for one
    for f in range(CONV2_OUT_CH):
        fr, fc = divmod(f, CONV2_GRID_COLS)
        cy = (fr * (fksz_y2 + FILTER_GAP)) - tgh2 / 2.0
        cz = (fc * (fksz_z2 + FILTER_GAP)) - tgw2 / 2.0
        cp = np.array([CONV2_X, cy, cz])
        conv2_filter_centers.append(cp)
        for in_ch in range(CONV2_IN_CH):  # Iterate through input channels
            for ky in range(CONV2_KERNEL_SIZE):
                for kx in range(CONV2_KERNEL_SIZE):
                    # Calculate position even if not spawning
                    x, y, z = (
                        CONV2_X,
                        cy
                        + (ky * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP))
                        - fksz_y2 / 2.0,
                        cz
                        + (kx * (WEIGHT_CUBE_SIZE + WEIGHT_CUBE_GAP))
                        - fksz_z2 / 2.0,
                    )
                    pos = np.array([x, y, z])
                    conv2_weight_positions[(f, in_ch, ky, kx)] = (
                        pos  # Store position by (filter, in_channel, ky, kx)
                    )
                    # Only spawn cubes for the first input channel to reduce clutter
                    if in_ch == 0:
                        commands.spawn(
                            Mesh3d(cube_mesh_handle),
                            MeshMaterial3d(weight_mat),
                            Transform.from_xyz(*pos).with_scale(Vec3.splat(0.0)),
                            Conv2WeightCube(
                                filter_idx=f, in_channel_idx=in_ch, ky=ky, kx=kx
                            ),
                        )

    detailed_conn_positions = []
    detailed_conn_colors = []
    vertex_count = 0
    start_v_i2c1, end_v_i2c1 = 0, 0
    start_v_c1c2, end_v_c1c2 = 0, 0
    start_v_c2o, end_v_c2o = 0, 0

    # Input -> Conv1 lines (Input cube -> Conv1 filter center) - unchanged
    start_v_i2c1 = vertex_count
    for i in range(len(input_positions)):
        for f in range(len(conv1_filter_centers)):
            detailed_conn_positions.extend(
                [input_positions[i], conv1_filter_centers[f]]
            )
            detailed_conn_colors.extend([[1.0, 1.0, 1.0, 0.01]] * 2)
            vertex_count += 2
    end_v_i2c1 = vertex_count

    # Conv1 -> Conv2 lines (Conv1 weight cube -> Conv2 weight cube) - NEW LOGIC
    start_v_c1c2 = vertex_count
    # Iterate through each Conv1 weight cube position
    for (f1, _ky1, _kx1), pos1 in conv1_weight_positions.items():
        # This Conv1 weight cube belongs to filter f1 (which is the input channel for Conv2)
        in_channel_for_conv2 = f1
        # Connect it to all Conv2 weight cubes that use this input channel
        for f2 in range(CONV2_OUT_CH):
            for ky2 in range(CONV2_KERNEL_SIZE):
                for kx2 in range(CONV2_KERNEL_SIZE):
                    # Get the position of the corresponding Conv2 weight cube
                    pos2 = conv2_weight_positions.get(
                        (f2, in_channel_for_conv2, ky2, kx2)
                    )
                    if pos2 is not None:  # Should always exist
                        detailed_conn_positions.extend([pos1, pos2])
                        detailed_conn_colors.extend([[1.0, 1.0, 1.0, 0.01]] * 2)
                        vertex_count += 2
    end_v_c1c2 = vertex_count

    # Conv2 -> Output lines (Conv2 weight cube -> Output cube) - unchanged geometry
    start_v_c2o = vertex_count
    in_ch_show = 0  # We only have positions for cubes with in_channel=0
    for f2_idx in range(CONV2_OUT_CH):
        for ky2 in range(CONV2_KERNEL_SIZE):
            for kx2 in range(CONV2_KERNEL_SIZE):
                source_pos = conv2_weight_positions.get((f2_idx, in_ch_show, ky2, kx2))
                if source_pos is None:
                    continue
                for o_idx in range(len(output_positions)):
                    dest_pos = output_positions[o_idx]
                    detailed_conn_positions.extend([source_pos, dest_pos])
                    detailed_conn_colors.extend([[1.0, 1.0, 1.0, 0.01]] * 2)
                    vertex_count += 2
    end_v_c2o = vertex_count

    detailed_conn_mesh = Mesh(PrimitiveTopology.LineList)
    detailed_conn_mesh.insert_attribute(
        Mesh.ATTRIBUTE_POSITION, np.array(detailed_conn_positions, dtype=np.float32)
    )
    detailed_conn_mesh.insert_attribute(
        Mesh.ATTRIBUTE_COLOR, np.array(detailed_conn_colors, dtype=np.float32)
    )

    detailed_conn_mesh_handle = meshes.add(detailed_conn_mesh)
    commands.insert_resource(
        DetailedConnectionMesh(
            mesh_handle=detailed_conn_mesh_handle,
            input_to_conv1_vert_range=(start_v_i2c1, end_v_i2c1),
            conv1_to_conv2_vert_range=(start_v_c1c2, end_v_c1c2),
            conv2_to_output_vert_range=(start_v_c2o, end_v_c2o),
        )
    )

    commands.spawn(
        Mesh3d(detailed_conn_mesh_handle),
        MeshMaterial3d(
            materials.add(StandardMaterial(unlit=True, alpha_mode=AlphaMode.Blend()))
        ),
    )

    commands.spawn(
        Camera3d(), Transform.from_xyz(0.0, 0.0, 15.0).looking_at(Vec3.ZERO, Vec3.Y)
    )


def train_cnn_system(
    time: Res[Time], bundle: ResMut[ModelBundleCNN], viz_state: ResMut[CnnVizState]
) -> None:
    try:
        inputs, labels = next(bundle.data_loader_iter)
    except StopIteration:
        data_loader_iter = create_mnist_loader()
        bundle.data_loader_iter = data_loader_iter
        inputs, labels = next(bundle.data_loader_iter)

    bundle.optimizer.zero_grad()
    outputs = bundle.model(inputs)  # Hooks save activations
    loss = bundle.criterion(outputs, labels)
    loss.backward()
    bundle.optimizer.step()

    viz_state.input_image = (inputs[0].data.cpu().numpy() * 0.5 + 0.5).squeeze()
    viz_state.output_activations = torch.softmax(outputs[0], dim=0).data.cpu().numpy()
    viz_state.conv1_weights = bundle.model.conv1.weight.data.cpu().numpy()
    viz_state.conv2_weights = bundle.model.conv2.weight.data.cpu().numpy()
    viz_state.fc1_weights = bundle.model.fc1.weight.data.cpu().numpy()


def update_input_cubes_system(
    viz_state: Res[CnnVizState], q_input: Query[tuple[Mut[Transform], InputCube]]
) -> None:
    pixels = viz_state.input_image.flatten()
    for transform, cube in q_input:
        scale = 0.01 + pixels[cube.index] * 0.29
        transform.scale = Vec3.splat(scale)


def update_output_cubes_system(
    viz_state: Res[CnnVizState], q_output: Query[tuple[Mut[Transform], OutputCube]]
) -> None:
    activations = viz_state.output_activations
    for transform, cube in q_output:
        scale = 0.1 + activations[cube.index] * 0.9
        transform.scale = Vec3(0.3, scale, 0.3)


def _weight_to_scale(w: float, max_abs: float) -> float:
    return 0.01 + (np.abs(w) / max_abs) * 0.19


def update_conv1_weights_system(
    viz_state: Res[CnnVizState],
    q_conv1: Query[tuple[Mut[Transform], Conv1WeightCube]],
) -> None:
    weights1 = viz_state.conv1_weights
    max_abs = max(np.max(np.abs(weights1)) if weights1.size > 0 else 0, 1e-6)
    for transform, cube in q_conv1:
        transform.scale = Vec3.splat(
            _weight_to_scale(weights1[cube.filter_idx, 0, cube.ky, cube.kx], max_abs)
        )


def update_conv2_weights_system(
    viz_state: Res[CnnVizState],
    q_conv2: Query[tuple[Mut[Transform], Conv2WeightCube]],
) -> None:
    weights2 = viz_state.conv2_weights
    max_abs = max(np.max(np.abs(weights2)) if weights2.size > 0 else 0, 1e-6)
    for transform, cube in q_conv2:
        if cube.in_channel_idx == 0:
            transform.scale = Vec3.splat(
                _weight_to_scale(
                    weights2[cube.filter_idx, cube.in_channel_idx, cube.ky, cube.kx], max_abs
                )
            )


def update_detailed_connections_system(
    viz_state: Res[CnnVizState],
    conn_res: Res[DetailedConnectionMesh],
    meshes: ResMut[Assets[Mesh]],
) -> None:
    conn_mesh = meshes.get_mut(conn_res.mesh_handle)
    assert conn_mesh is not None, "Detailed connection mesh not found"

    input_acts = viz_state.input_image.flatten()
    conv1_acts = viz_state.conv1_activations
    conv2_acts = viz_state.conv2_pooled_activations
    output_acts = viz_state.output_activations
    weights2 = viz_state.conv2_weights
    fc1_weights = viz_state.fc1_weights

    avg_act1_per_filter = np.mean(conv1_acts, axis=(1, 2))
    avg_act2_per_filter = np.mean(conv2_acts, axis=(1, 2))
    avg_fc1_weight_per_conv2_filter = np.mean(
        fc1_weights.reshape(FC1_OUT_FEATURES, CONV2_OUT_CH, 14 * 14), axis=(0, 2)
    )

    max_avg_act1 = np.max(avg_act1_per_filter) if avg_act1_per_filter.size > 0 else 1e-6
    max_avg_act2 = np.max(avg_act2_per_filter) if avg_act2_per_filter.size > 0 else 1e-6
    max_output_act = np.max(output_acts) if output_acts.size > 0 else 1e-6
    max_abs_w2 = np.max(np.abs(weights2)) if weights2.size > 0 else 1e-6

    # Increase power further for steeper falloff
    HIGHER_POWER = 6.0  # Was 4.0
    # Reduce the maximum possible alpha values significantly
    MAX_ALPHA_DEST = 0.2  # Was ~0.4-0.5
    MAX_ALPHA_OUT = 0.35  # Was ~0.6-0.7
    MIN_ALPHA = 0.001  # Make minimum alpha even lower

    def map_avg_to_alpha(
        avg_act: float | np.floating | np.ndarray,  # type: ignore[type-arg]
        max_avg: float | np.floating | np.ndarray,  # type: ignore[type-arg]
        min_alpha: float = MIN_ALPHA,
        max_alpha: float = MAX_ALPHA_DEST,
        power: float = HIGHER_POWER,
    ) -> float | np.ndarray:  # type: ignore[type-arg]
        norm_act = np.clip(avg_act / max(max_avg, 1e-6), 0.0, 1.0)
        scaled_act = norm_act**power
        alpha = min_alpha + scaled_act * (max_alpha - min_alpha)
        return np.clip(alpha, min_alpha, max_alpha)


    # Adjust source scaling max alpha too
    def scale_source_alpha(
        avg_act: float, act_scale_factor: float = 1.0, min_alpha_scale: float = 0.05, max_alpha_scale: float = 0.8
    ) -> float:  # Reduced max_alpha_scale
        return np.clip(
            min_alpha_scale + avg_act * act_scale_factor,
            min_alpha_scale,
            max_alpha_scale,
        )

    # Apply the modified mapping function with new power and max alpha values
    alpha1_dest_per_filter = map_avg_to_alpha(
        avg_act1_per_filter, max_avg_act1, max_alpha=MAX_ALPHA_DEST, power=HIGHER_POWER
    )
    alpha2_dest_per_filter = map_avg_to_alpha(
        avg_act2_per_filter, max_avg_act2, max_alpha=MAX_ALPHA_DEST, power=HIGHER_POWER
    )
    alpha3_dest_per_output = map_avg_to_alpha(
        output_acts, max_output_act, max_alpha=MAX_ALPHA_OUT, power=HIGHER_POWER
    )

    with conn_mesh.attribute_mut(Mesh.ATTRIBUTE_COLOR) as data:
        v_idx = 0
        RED = np.array([1.0, 0.2, 0.2])
        BLUE = np.array([0.2, 0.2, 1.0])
        WHITE = np.array([1.0, 1.0, 1.0])

        # Input -> Conv1
        start, end = conn_res.input_to_conv1_vert_range
        for i in range(MNIST_IMG_SIZE * MNIST_IMG_SIZE):
            input_alpha_scale = MIN_ALPHA + (input_acts[i] ** HIGHER_POWER) * (
                1.0 - MIN_ALPHA
            )
            for f in range(CONV1_OUT_CH):
                alpha = alpha1_dest_per_filter[f] * input_alpha_scale  # type: ignore[index]
                data[start + v_idx, 0:3] = WHITE
                data[start + v_idx, 3] = alpha
                data[start + v_idx + 1, 0:3] = WHITE
                data[start + v_idx + 1, 3] = alpha
                v_idx += 2

        # Conv1 -> Conv2
        v_idx = 0
        start, end = conn_res.conv1_to_conv2_vert_range
        for f1_idx in range(CONV1_OUT_CH):
            source_alpha_scale = map_avg_to_alpha(
                avg_act1_per_filter[f1_idx],
                max_avg_act1,
                min_alpha=0.1,
                max_alpha=1.0,
                power=HIGHER_POWER,
            )
            for _ky1 in range(CONV1_KERNEL_SIZE):
                for _kx1 in range(CONV1_KERNEL_SIZE):
                    for f2_idx in range(CONV2_OUT_CH):
                        for _ky2 in range(CONV2_KERNEL_SIZE):
                            for _kx2 in range(CONV2_KERNEL_SIZE):
                                alpha = (
                                    alpha2_dest_per_filter[f2_idx] * source_alpha_scale  # type: ignore[index]
                                )
                                data[start + v_idx, 0:3] = WHITE
                                data[start + v_idx, 3] = alpha
                                data[start + v_idx + 1, 0:3] = WHITE
                                data[start + v_idx + 1, 3] = alpha
                                v_idx += 2

        # Conv2 -> Output
        v_idx = 0
        start, end = conn_res.conv2_to_output_vert_range
        in_ch_show = 0
        for f2 in range(CONV2_OUT_CH):
            avg_outgoing_weight = avg_fc1_weight_per_conv2_filter[f2]
            color = RED if avg_outgoing_weight > 0 else BLUE
            source_alpha_scale = map_avg_to_alpha(
                avg_act2_per_filter[f2],
                max_avg_act2,
                min_alpha=0.1,
                max_alpha=1.0,
                power=HIGHER_POWER,
            )

            for ky in range(CONV2_KERNEL_SIZE):
                for kx in range(CONV2_KERNEL_SIZE):
                    weight_val = weights2[f2, in_ch_show, ky, kx]
                    weight_alpha_scale = np.clip(
                        0.1 + (np.abs(weight_val) / max_abs_w2) * 0.9, 0.1, 1.0
                    )
                    for o in range(NUM_OUTPUTS):
                        alpha = (
                            alpha3_dest_per_output[o]  # type: ignore[index]
                            * source_alpha_scale
                            * weight_alpha_scale
                        )
                        data[start + v_idx, 0:3] = color
                        data[start + v_idx, 3] = alpha
                        data[start + v_idx + 1, 0:3] = color
                        data[start + v_idx + 1, 3] = alpha
                        v_idx += 2


def rotate_camera_system(time: Res[Time], query: Query[Mut[Transform], With[Camera3d]]) -> None:
    for transform in query:
        rotation = Quat.from_rotation_y(0.3 * time.delta_secs())
        transform.rotate_around(Vec3.ZERO, rotation)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.BLACK))  # Black background
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                train_cnn_system,
                update_input_cubes_system,
                update_output_cubes_system,
                update_conv1_weights_system,
                update_conv2_weights_system,
                update_detailed_connections_system,
                rotate_camera_system,
            ),
        )
    )


if __name__ == "__main__":
    main().run()
