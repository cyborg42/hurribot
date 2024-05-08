use plotters::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root_area = BitMapBackend::new("./logs/line_chart.png", (640, 480)).into_drawing_area();
    root_area.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root_area)
        .caption("Line Chart", ("sans-serif", 50).into_font())
        .x_label_area_size(35)
        .y_label_area_size(40)
        .build_cartesian_2d(0..10, 0..10)?;

    chart.configure_mesh().draw()?;

    chart
        .draw_series(LineSeries::new(
            (0..10).map(|x| (x, x)), // 用一些点创建线性图
            &RED,
        ))?
        .label("Linear")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));

    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()?;

    Ok(())
}
