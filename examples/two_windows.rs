use bevy::{
    core_pipeline::{self, node, AlphaMask3d, Opaque3d, Transparent3d},
    prelude::*,
    render::{
        camera::{ActiveCamera, CameraTypePlugin, RenderTarget},
        render_graph::{self, NodeRunError, RenderGraph, RenderGraphContext, SlotValue},
        render_phase::RenderPhase,
        renderer::RenderContext,
        RenderApp, RenderStage,
    },
    window::{CreateWindow, PresentMode, WindowId},
    winit::WinitSettings,
};
use bevy_egui::{EguiContext, EguiPlugin};
use once_cell::sync::Lazy;

static SECOND_WINDOW_ID: Lazy<WindowId> = Lazy::new(WindowId::new);

struct Images {
    bevy_icon: Handle<Image>,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_plugin(SecondWindowCameraPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .init_resource::<SharedUiState>()
        .add_startup_system(load_assets)
        .add_startup_system(create_new_window)
        .add_system(ui_first_window)
        .add_system(ui_second_window)
        .run();
}

const SECONDARY_PASS_DRIVER: &str = "secondary_pass_driver";
const SECONDARY_EGUI_PASS: &str = "secondary_egui_pass";

fn load_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Images {
        bevy_icon: assets.load("icon.png"),
    });
}

#[derive(Default)]
struct UiState {
    input: String,
}

#[derive(Default)]
struct SharedUiState {
    shared_input: String,
}

fn ui_first_window(
    mut egui_context: ResMut<EguiContext>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
    images: Res<Images>,
) {
    let bevy_texture_id = egui_context.add_image(images.bevy_icon.clone_weak());
    egui::Window::new("First Window")
        .vscroll(true)
        .show(egui_context.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut ui_state.input);
            });
            ui.horizontal(|ui| {
                ui.label("Shared input: ");
                ui.text_edit_singleline(&mut shared_ui_state.shared_input);
            });

            ui.add(egui::widgets::Image::new(bevy_texture_id, [256.0, 256.0]));
        });
}

fn ui_second_window(
    mut egui_context: ResMut<EguiContext>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
    images: Res<Images>,
) {
    let bevy_texture_id = egui_context.add_image(images.bevy_icon.clone_weak());
    let ctx = match egui_context.try_ctx_for_window_mut(*SECOND_WINDOW_ID) {
        Some(ctx) => ctx,
        None => return,
    };
    egui::Window::new("Second Window")
        .vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Write something else: ");
                ui.text_edit_singleline(&mut ui_state.input);
            });
            ui.horizontal(|ui| {
                ui.label("Shared input: ");
                ui.text_edit_singleline(&mut shared_ui_state.shared_input);
            });

            ui.add(egui::widgets::Image::new(bevy_texture_id, [256.0, 256.0]));
        });
}

struct SecondWindowCameraPlugin;
impl Plugin for SecondWindowCameraPlugin {
    fn build(&self, app: &mut App) {
        // adds the `ActiveCamera<SecondWindowCamera3d>` resource and extracts the camera into the render world
        app.add_plugin(CameraTypePlugin::<SecondWindowCamera3d>::default());

        let render_app = app.sub_app_mut(RenderApp);

        // add `RenderPhase<Opaque3d>`, `RenderPhase<AlphaMask3d>` and `RenderPhase<Transparent3d>` camera phases
        render_app.add_system_to_stage(RenderStage::Extract, extract_second_camera_phases);

        // add a render graph node that executes the 3d subgraph
        let mut render_graph = render_app.world.resource_mut::<RenderGraph>();
        let second_window_node = render_graph.add_node("second_window_cam", SecondWindowDriverNode);
        render_graph
            .add_node_edge(
                core_pipeline::node::MAIN_PASS_DEPENDENCIES,
                second_window_node,
            )
            .unwrap();
        render_graph
            .add_node_edge(core_pipeline::node::CLEAR_PASS_DRIVER, second_window_node)
            .unwrap();

        render_graph
            .add_node_edge(node::MAIN_PASS_DEPENDENCIES, SECONDARY_PASS_DRIVER)
            .unwrap();

        bevy_egui::setup_pipeline(
            &mut render_graph,
            bevy_egui::RenderGraphConfig {
                window_id: *SECOND_WINDOW_ID,
                egui_pass: SECONDARY_EGUI_PASS,
            },
        );
    }
}

struct SecondWindowDriverNode;
impl render_graph::Node for SecondWindowDriverNode {
    fn run(
        &self,
        graph: &mut RenderGraphContext,
        _: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if let Some(camera) = world.resource::<ActiveCamera<SecondWindowCamera3d>>().get() {
            graph.run_sub_graph(
                core_pipeline::draw_3d_graph::NAME,
                vec![SlotValue::Entity(camera)],
            )?;
        }

        Ok(())
    }
}

fn extract_second_camera_phases(
    mut commands: Commands,
    active: Res<ActiveCamera<SecondWindowCamera3d>>,
) {
    if let Some(entity) = active.get() {
        commands.get_or_spawn(entity).insert_bundle((
            RenderPhase::<Opaque3d>::default(),
            RenderPhase::<AlphaMask3d>::default(),
            RenderPhase::<Transparent3d>::default(),
        ));
    }
}

#[derive(Component, Default)]
struct SecondWindowCamera3d;

fn create_new_window(mut create_window_events: EventWriter<CreateWindow>, mut commands: Commands) {
    // sends out a "CreateWindow" event, which will be received by the windowing backend
    create_window_events.send(CreateWindow {
        id: SECOND_WINDOW_ID.to_owned(),
        descriptor: WindowDescriptor {
            width: 800.,
            height: 600.,
            present_mode: PresentMode::Immediate,
            title: "Second window".to_string(),
            ..default()
        },
    });

    // second window camera
    commands.spawn_bundle(PerspectiveCameraBundle {
        camera: Camera {
            target: RenderTarget::Window(SECOND_WINDOW_ID.to_owned()),
            ..default()
        },
        transform: Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        marker: SecondWindowCamera3d,
        ..PerspectiveCameraBundle::new()
    });
}
