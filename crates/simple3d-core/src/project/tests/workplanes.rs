//! Workplanes saved with the component they were put down in (issue 125).

use super::*;
use crate::scene::Workplane;

fn workplane(name: &str) -> Workplane {
    Workplane {
        id: 0,
        name: name.into(),
        base: Vec3::new(10.0, 0.0, 10.0),
        base_rotation: Vec3::new(90.0, 0.0, 90.0),
        offset: [1.5, -2.0, 0.5],
        spin: 30.0,
        spacing: 2.5,
    }
}

#[test]
pub(crate) fn workplanes_and_the_active_one_survive_saving_in_every_component() {
    let mut root = sample();
    let side = root.settings.add_workplane(workplane("Side"));
    root.settings.add_workplane(workplane("Top"));
    root.settings.active_workplane = Some(side);
    let mut part = Scene::new();
    let own = part.settings.add_workplane(workplane("Part"));
    let text = project_to_string(&root, &[(7, &part)]);
    let back = project_from_str(&text).expect("round trip");
    assert_eq!(back.root.settings.workplanes, root.settings.workplanes);
    assert_eq!(back.root.settings.active_workplane, Some(side));
    let (_, read) = back.components.iter().find(|(id, _)| *id == 7).expect("the component was lost");
    assert_eq!(read.settings.workplanes, part.settings.workplanes, "a component lost its own workplanes");
    assert_eq!(read.settings.workplane(own).map(|plane| plane.name.as_str()), Some("Part"));
}

#[test]
pub(crate) fn a_project_without_workplanes_writes_nothing_for_them() {
    // So files from before issue 125 are unchanged by a save.
    let text = to_string(&sample());
    assert!(!text.contains("workplane"), "an empty list was written out");
}
