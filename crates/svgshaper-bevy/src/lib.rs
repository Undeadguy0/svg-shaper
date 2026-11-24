//! Тонкий плагин Bevy 0.20 поверх [`svgshaper`].
//!
//! [`SvgShaperPlugin::build`] регистрирует сообщение [`ConversionFinished`] и
//! систему, которая забирает очередь. Ресурсы плагин сам не создаёт и процесс
//! растеризации не собирает: рисунок остаётся в ядре.
//!
//! Хост кладёт в мир [`PendingConversions`]. Система порождает
//! [`svgshaper::convert`] в пуле задач Bevy и, когда future готова, публикует
//! `Result` ядра через [`bevy::prelude::Commands`]. Пока future не готова,
//! система её не дожидается.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;

use svgshaper::{ConversionReport, ConversionRequest, RequestError, StopFlag, convert};

/// Итог одного запроса ядра. Ровно одно такое сообщение на взятый запрос.
#[derive(Message, Clone, Debug)]
pub struct ConversionFinished(pub Result<ConversionReport, RequestError>);

/// Очередь, которую наполняет хост. Плагин её не создаёт.
#[derive(Resource, Default)]
pub struct PendingConversions {
    pub queue: Vec<(ConversionRequest, StopFlag)>,
}

/// Регистрирует сообщение итога и систему опроса. Конвертацию сам не начинает.
pub struct SvgShaperPlugin;

impl Plugin for SvgShaperPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ConversionFinished>()
            .add_systems(Update, pump_conversions);
    }
}

type Job = Pin<Box<dyn Future<Output = Result<ConversionReport, RequestError>> + Send>>;

fn pump_conversions(
    pending: Option<ResMut<PendingConversions>>,
    mut jobs: Local<Vec<Job>>,
    mut commands: Commands,
) {
    if let Some(mut pending) = pending {
        let pool = AsyncComputeTaskPool::get();
        for (request, stop) in pending.queue.drain(..) {
            let task = pool.spawn(async move { convert(request, stop).await });
            jobs.push(Box::pin(task));
        }
    }

    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut index = 0;
    while index < jobs.len() {
        match jobs[index].as_mut().poll(&mut context) {
            Poll::Ready(result) => {
                drop(jobs.swap_remove(index));
                commands.queue(move |world: &mut World| {
                    world.write_message(ConversionFinished(result));
                });
            }
            Poll::Pending => index += 1,
        }
    }
}
