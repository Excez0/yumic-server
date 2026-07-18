use gtk::prelude::*;
use gtk::cairo;
use std::rc::Rc;
use std::cell::Cell;

impl Clone for AudioMeter {
    fn clone(&self) -> Self {
        Self {
            widget: self.widget.clone(),
            level: Rc::clone(&self.level),
            peak: Rc::clone(&self.peak),
        }
    }
}

pub struct AudioMeter {
    pub widget: gtk::DrawingArea,
    level: Rc<Cell<f64>>,
    peak: Rc<Cell<f64>>,
}

impl AudioMeter {
    pub fn new() -> Self {
        let level = Rc::new(Cell::new(0.0_f64));
        let peak = Rc::new(Cell::new(0.0_f64));

        let widget = gtk::DrawingArea::builder()
            .content_width(300)
            .content_height(28)
            .hexpand(true)
            .height_request(28)
            .css_classes(["audio-meter"])
            .build();

        let level_cb = Rc::clone(&level);
        let peak_cb = Rc::clone(&peak);

        widget.set_draw_func(move |_area, cr, w, h| {
            let w = w as f64;
            let h = h as f64;
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            draw_meter(cr, w, h, level_cb.get(), peak_cb.get());
        });

        Self { widget, level, peak }
    }

    pub fn set_level(&self, level: f64) {
        self.level.set(level.clamp(0.0, 1.0));
        self.widget.queue_draw();
    }

    pub fn set_peak(&self, peak: f64) {
        self.peak.set(peak.clamp(0.0, 1.0));
        self.widget.queue_draw();
    }
}

fn draw_meter(cr: &cairo::Context, w: f64, h: f64, level: f64, peak: f64) {
    // Background
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.2);
    rounded_rect(cr, 0.0, 0.0, w, h, h / 2.0);
    cr.fill().ok();

    // Level bar
    if level > 0.001 {
        let bar_x = 2.0;
        let bar_w = (w - 4.0) * level;
        let bar_y = 2.0;
        let bar_h = h - 4.0;
        let r = (bar_h / 2.0).max(0.0);

        let grad = cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
        grad.add_color_stop_rgb(0.0, 0.26, 0.76, 0.38);
        grad.add_color_stop_rgb(0.5, 0.96, 0.77, 0.11);
        grad.add_color_stop_rgb(0.78, 0.95, 0.42, 0.07);
        grad.add_color_stop_rgb(1.0, 0.89, 0.19, 0.19);

        let _ = cr.set_source(&grad);
        rounded_rect(cr, bar_x, bar_y, bar_w, bar_h, r);
        cr.fill().ok();

        let hl = cairo::LinearGradient::new(0.0, bar_y, 0.0, bar_y + bar_h);
        hl.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.3);
        hl.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.0);
        let _ = cr.set_source(&hl);
        rounded_rect(cr, bar_x, bar_y, bar_w, bar_h, r);
        cr.fill().ok();
    }

    // Peak indicator
    if peak > 0.001 {
        let px = 2.0 + (w - 4.0) * peak;
        let (r, g, b) = if peak > 0.9 {
            (0.89, 0.19, 0.19)
        } else if peak > 0.7 {
            (0.95, 0.42, 0.07)
        } else if peak > 0.5 {
            (0.96, 0.77, 0.11)
        } else {
            (0.26, 0.76, 0.38)
        };
        cr.set_source_rgba(r, g, b, 0.9);
        cr.rectangle(px - 1.5, 1.0, 3.0, h - 2.0);
        cr.fill().ok();
    }
}

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    if r < 0.5 {
        cr.rectangle(x, y, w, h);
        return;
    }
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();
}
