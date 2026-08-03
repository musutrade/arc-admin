import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { Router } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';

@Component({
  selector: 'app-error-500',
  imports: [MatIconModule],
  templateUrl: './error-500.html',
  styleUrl: './error-pages.scss',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class Error500Page {
  private readonly router = inject(Router);

  reload(): void {
    window.location.reload();
  }

  goHome(): void {
    this.router.navigate(['/permissions']);
  }
}
