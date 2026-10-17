// Puts a play button over each demo video until it plays, so it reads as a video at a glance.
// Runs on every page, including those the instant navigation loads.
document$.subscribe(() => {
  for (const video of document.querySelectorAll('video.demo-video')) {
    if (video.parentElement.classList.contains('video-frame')) continue;
    const frame = document.createElement('div');
    frame.className = 'video-frame';
    video.replaceWith(frame);
    frame.appendChild(video);
    const play = document.createElement('button');
    play.type = 'button';
    play.className = 'video-play';
    play.setAttribute('aria-label', 'Play the video');
    frame.appendChild(play);
    play.addEventListener('click', () => video.play());
    const update = () => frame.classList.toggle('playing', !video.paused);
    video.addEventListener('play', update);
    video.addEventListener('pause', update);
    video.addEventListener('ended', update);
  }
});
