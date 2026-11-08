//! The default content (#1893): the bundled bot takes damage through
//! `OnTakeDamage`, plays `Hit`, then `Death`, and stops chasing once dead.

use rusty::dev::harness::Harness;

use crate::play::eval;

#[test]
fn the_bundled_bot_takes_damage_and_dies() {
    let h = Harness::new(crate::temp::dir().join("bot"), "assets/scripts/bot.lua");
    h.step(2);
    let bot = eval(&h, "return Scene.FindEntityByName('Enemy_1')");
    let hit = format!(
        "local taken = Damage.Apply({bot}, 30, {{ type = 'bullet' }}) \
         return taken, Scene.GetScript({bot}, 'bot').health, Animator.GetClip({bot})"
    );
    assert_eq!(eval(&h, &hit), "30, 70, Hit");
    let kill = format!(
        "Damage.Apply({bot}, 80) \
         return Damage.Apply({bot}, 5), Animator.GetClip({bot})"
    );
    assert_eq!(eval(&h, &kill), "0, Death", "a dead bot takes no more");
    h.step(5);
    assert_eq!(
        eval(&h, &format!("return Animator.GetClip({bot})")),
        "Death",
        "its Update no longer walks over the death clip"
    );
}
